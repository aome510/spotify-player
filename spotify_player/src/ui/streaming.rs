use crate::state::SharedState;
use librespot_playback::{
    audio_backend::{Sink, SinkResult},
    convert::Converter,
    decoder::AudioPacket,
};
use parking_lot::Mutex;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Bar, BarChart, BarGroup},
    Frame,
};
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

const FFT_SIZE: usize = 1024;
/// Number of new samples consumed per FFT frame (overlap = `FFT_SIZE` - `HOP_SIZE`).
/// At 44100 Hz: 128 samples ≈ 2.9 ms between updates.
const HOP_SIZE: usize = 128;
pub const NUM_BANDS: usize = 128;

/// Height (in terminal rows) reserved for the audio visualization bar chart.
pub const VIS_HEIGHT: u16 = 8;

/// Per-FFT-frame decay multiplier for individual bands.
/// At 44100 Hz / `HOP_SIZE` 128 ≈ 344 hops/s, 0.985^~151 ≈ 1% in ~0.44 s — snappy
/// enough to track transients, not so slow that it smears.
const DECAY_FACTOR: f32 = 0.985;
/// Slower decay for the peak envelope used for normalization. At 344 hops/s,
/// 0.9985^x = 0.01 → x ≈ 1535 hops → ~4.5 s. The envelope stays elevated
/// through quiet passages so the bars reflect genuine relative loudness instead
/// of always filling to 100%.
const DECAY_FACTOR_PEAK: f32 = 0.9985;
const SAMPLE_RATE: f32 = 44100.0;

/// Shared frequency-band state exposed between the audio sink and the UI.
/// Storing `updated_at` lets the render function apply smooth time-based decay
/// independent of how often `write()` is called by the audio backend.
pub struct VisBands {
    pub values: Vec<f32>,
    pub updated_at: Instant,
    /// Slow-decaying peak envelope used to normalise bar heights.
    /// Rises instantly to any louder value; decays with `DECAY_FACTOR_PEAK`.
    /// Kept separate from per-band values so quiet passages look genuinely
    /// quieter — the VU «breathes» with the music.
    pub peak_envelope: f32,
}

impl VisBands {
    pub fn new() -> Self {
        Self {
            values: vec![0.0f32; NUM_BANDS],
            updated_at: Instant::now(),
            peak_envelope: 1e-6,
        }
    }
}

/// Returns the compound `DECAY_FACTOR` multiplier for the given elapsed wall-clock
/// duration, calibrated to the same per-hop rate used in the audio sink.
pub fn decay_for_elapsed(elapsed: std::time::Duration) -> f32 {
    let elapsed_hops = elapsed.as_secs_f32() * SAMPLE_RATE / HOP_SIZE as f32;
    DECAY_FACTOR.powf(elapsed_hops)
}

/// Same as `decay_for_elapsed` but uses the slower peak-envelope decay rate.
pub fn peak_decay_for_elapsed(elapsed: std::time::Duration) -> f32 {
    let elapsed_hops = elapsed.as_secs_f32() * SAMPLE_RATE / HOP_SIZE as f32;
    DECAY_FACTOR_PEAK.powf(elapsed_hops)
}

/// An audio sink wrapper that computes real-time FFT frequency bands from the
/// decoded audio stream and exposes them via a shared buffer for the UI.
///
/// It forwards every audio packet unchanged to the real backend, so playback
/// is not affected.
pub struct VisualizationSink {
    inner: Box<dyn Sink>,
    /// Ring-buffer of mono f32 samples waiting to be processed.
    /// `VecDeque` gives O(1) front-drain instead of Vec's O(remaining) shift.
    sample_buf: VecDeque<f32>,
    bands: Arc<Mutex<VisBands>>,
    fft: Arc<dyn rustfft::Fft<f32>>,
}

impl VisualizationSink {
    pub fn new(inner: Box<dyn Sink>, bands: Arc<Mutex<VisBands>>) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        Self {
            inner,
            sample_buf: VecDeque::with_capacity(FFT_SIZE * 2),
            bands,
            fft,
        }
    }
}

impl Sink for VisualizationSink {
    fn start(&mut self) -> SinkResult<()> {
        self.inner.start()
    }

    fn stop(&mut self) -> SinkResult<()> {
        // Zero out the bands when playback stops so the bars fall to silence.
        let mut g = self.bands.lock();
        g.values.fill(0.0);
        g.updated_at = Instant::now();
        drop(g);
        self.sample_buf.clear();
        self.inner.stop()
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        if let AudioPacket::Samples(ref samples) = packet {
            // Samples are interleaved stereo (L, R, L, R, …); mix down to mono f32.
            self.sample_buf.extend(samples.chunks(2).map(|c| {
                if c.len() == 2 {
                    f64::midpoint(c[0], c[1]) as f32
                } else {
                    c[0] as f32
                }
            }));

            // Update vis_bands after EVERY hop (not at the end of the batch).
            //
            // Batching reduces mutex contention but delays the first update by the
            // full packet duration (~46 ms for a 2048-sample packet). With per-hop
            // updates a transient at the START of a packet is visible within one hop
            // (~2.9 ms) + render delay (~32 ms) instead of ~78 ms.
            //
            // Decay correctness is preserved by using wall-clock elapsed from
            // vis_bands.updated_at: hops within the same write() call are ~3 ms apart
            // so decay_for_elapsed(3 ms) ≈ 0.985 ≈ 1.0 — no peak smearing.
            while self.sample_buf.len() >= FFT_SIZE {
                let window = self.sample_buf.make_contiguous();

                // Apply a Hann window to reduce spectral leakage.
                let mut fft_buf: Vec<Complex<f32>> = window[..FFT_SIZE]
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| {
                        let w = 0.5
                            * (1.0
                                - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32)
                                    .cos());
                        Complex::new(s * w, 0.0)
                    })
                    .collect();

                self.fft.process(&mut fft_buf);

                let magnitudes: Vec<f32> =
                    fft_buf[..FFT_SIZE / 2].iter().map(|c| c.norm()).collect();

                let mut new_bands = compute_log_bands(&magnitudes, FFT_SIZE / 2, NUM_BANDS);
                smooth_bands(&mut new_bands);

                // Apply wall-clock decay since the last hop, then rise to any louder value.
                let mut g = self.bands.lock();
                let decay = decay_for_elapsed(g.updated_at.elapsed());
                let peak_decay = peak_decay_for_elapsed(g.updated_at.elapsed());
                let frame_peak = new_bands.iter().copied().fold(0.0_f32, f32::max);
                for (stored, fresh) in g.values.iter_mut().zip(new_bands.iter()) {
                    *stored = (*stored * decay).max(*fresh);
                }
                g.peak_envelope = (g.peak_envelope * peak_decay).max(frame_peak);
                g.updated_at = Instant::now();
                drop(g);

                self.sample_buf.drain(..HOP_SIZE);
            }
        }

        self.inner.write(packet, converter)
    }
}

/// Groups FFT magnitude bins into `num_bands` bands on a logarithmic frequency
/// scale and returns the RMS magnitude of each band.
///
/// At the low-frequency end the log scale has fewer distinct bins than bands
/// (e.g. with `FFT_SIZE=1024` and `NUM_BANDS=128`, bins 0-13 would all map to bin 1).
/// We track `used_up_to` so that every band starts where the previous one ended,
/// preventing the duplicate-bin plateau that otherwise appears on the left side.
fn compute_log_bands(magnitudes: &[f32], num_bins: usize, num_bands: usize) -> Vec<f32> {
    let log_min = 1.0_f64;
    let log_max = num_bins as f64;

    // Next bin index that hasn't been assigned to a band yet.
    // Starts at 1 to skip bin 0 (DC component).
    let mut used_up_to: usize = 1;

    (0..num_bands)
        .map(|band| {
            let t_start = band as f64 / num_bands as f64;
            let t_end = (band + 1) as f64 / num_bands as f64;

            let natural_start = (log_min * (log_max / log_min).powf(t_start)) as usize;
            let natural_end = (log_min * (log_max / log_min).powf(t_end)) as usize;

            // Advance past already-used bins so low-frequency bands do not all
            // share the same FFT bin and produce an identical flat plateau.
            let start = natural_start.max(used_up_to).min(num_bins - 1);

            // All bins exhausted — pad with silence.
            if used_up_to >= num_bins {
                return 0.0;
            }

            let end = natural_end.max(start + 1).min(num_bins);
            used_up_to = end;

            let len = (end - start) as f32;
            let sum_sq: f32 = magnitudes[start..end].iter().map(|&v| v * v).sum();
            (sum_sq / len).sqrt()
        })
        .collect()
}

/// Applies a single pass of 3-point weighted smoothing [0.25, 0.5, 0.25] across
/// adjacent bands to reduce per-bin jitter without blurring transients.
fn smooth_bands(bands: &mut [f32]) {
    if bands.len() < 3 {
        return;
    }
    let copy: Vec<f32> = bands.to_vec();
    for i in 0..bands.len() {
        let prev = if i > 0 { copy[i - 1] } else { copy[0] };
        let next = if i + 1 < copy.len() {
            copy[i + 1]
        } else {
            copy[copy.len() - 1]
        };
        bands[i] = prev * 0.25 + copy[i] * 0.5 + next * 0.25;
    }
}

/// Maps a normalised amplitude [0, 1] to an RGB colour.
/// Quiet (0.0) → cool blue, medium → green, loud (1.0) → hot red.
fn bar_color(t: f32) -> Color {
    let (r, g, b) = if t < 0.5 {
        let s = t * 2.0;
        (
            (30.0 + 20.0 * s) as u8,
            (100.0 + 155.0 * s) as u8,
            (255.0 * (1.0 - s * 0.5)) as u8,
        )
    } else {
        let s = (t - 0.5) * 2.0;
        (
            (50.0 + 205.0 * s) as u8,
            (255.0 * (1.0 - s)) as u8,
            (128.0 * (1.0 - s)) as u8,
        )
    };
    Color::Rgb(r, g, b)
}

/// Render a frequency-band bar chart using live FFT data from the audio sink.
///
/// Bars are subsampled to the available rect width so they always fill the area
/// cleanly. Heights use a sqrt (perceptual) curve so quiet signals stay visible.
/// Each bar is coloured by its amplitude: cool blue (quiet) → green → hot red (loud).
pub fn render_audio_visualization(frame: &mut Frame, state: &SharedState, rect: Rect) {
    // display_decay interpolates bar heights smoothly between write() calls.
    // We normalise against peak_envelope (NOT the per-frame peak), so display_decay
    // no longer cancels out and bars genuinely fade between audio packets.
    let guard = state.vis_bands.lock();
    let display_decay = decay_for_elapsed(guard.updated_at.elapsed());
    let peak_norm =
        (guard.peak_envelope * peak_decay_for_elapsed(guard.updated_at.elapsed())).max(1e-6);
    let values = guard.values.clone();
    drop(guard);
    let num_bars = (rect.width as usize).min(values.len()).max(1);
    // Multiply by 8 to use ratatui's eighth-block characters (▁▂▃▄▅▆▇█),
    // giving 8× the resolution of whole terminal rows.
    let max_val = u64::from(rect.height) * 8;

    let step = values.len() as f64 / num_bars as f64;
    let bars: Vec<Bar> = (0..num_bars)
        .map(|i| {
            let idx = ((i as f64 * step) as usize).min(values.len() - 1);
            // Normalise against the slow peak envelope, then apply inter-frame decay.
            // Sqrt (gamma 0.5) scaling boosts quiet signals without clipping louds.
            let norm = ((values[idx] * display_decay) / peak_norm)
                .clamp(0.0, 1.0)
                .powf(0.5);
            let val = (norm * max_val as f32) as u64;
            Bar::default()
                .value(val)
                .text_value(String::new())
                .style(Style::default().fg(bar_color(norm)))
        })
        .collect();

    let chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width(1)
        .bar_gap(0)
        .max(max_val);

    frame.render_widget(chart, rect);
}
