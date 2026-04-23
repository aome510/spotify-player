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
/// Reference sample rate used by the **render-side** decay helpers
/// (`decay_for_elapsed`, `peak_decay_for_elapsed`).
/// The audio sink uses its own `VisualizationSink::sample_rate` field so that
/// decay timings stay precise if librespot streams at 48 000 Hz instead.
const SAMPLE_RATE: f32 = 44_100.0;

/// Shared frequency-band state exposed between the audio sink and the UI.
/// Storing `updated_at` lets the render function apply smooth time-based decay
/// independent of how often `write()` is called by the audio backend.
pub struct VisBands {
    /// Fixed-size array of per-band magnitudes.
    /// Using `[f32; NUM_BANDS]` instead of `Vec<f32>` means the render-frame
    /// copy (`let values = guard.values;`) is a plain stack copy with no heap
    /// allocation.
    pub values: [f32; NUM_BANDS],
    /// Wall-clock timestamp of the last `write()` hop that updated `values`.
    /// The render function reads this to compute time-based inter-frame decay
    /// without needing to be called at a fixed rate.
    pub updated_at: Instant,
    /// Slow-decaying peak envelope used to normalise bar heights.
    /// Rises instantly to any louder value; decays with `DECAY_FACTOR_PEAK`.
    /// Kept separate from per-band values so quiet passages look genuinely
    /// quieter — the VU «breathes» with the music.
    pub peak_envelope: f32,
    /// Set to `true` when librespot reports a `Playing` event and `false` on
    /// `Paused` or `stop()`.  The UI uses this flag to skip rendering (and
    /// reclaim the screen space) when audio is not being streamed locally.
    pub is_active: bool,
}

impl VisBands {
    pub fn new() -> Self {
        Self {
            values: [0.0f32; NUM_BANDS],
            updated_at: Instant::now(),
            peak_envelope: 1e-6,
            is_active: false,
        }
    }
}

impl Default for VisBands {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the compound `DECAY_FACTOR` multiplier for the given elapsed wall-clock
/// duration.
///
/// Used **only on the render side** (`render_audio_visualization`) to interpolate
/// bar heights smoothly between audio-sink updates. It uses the fixed `SAMPLE_RATE`
/// reference (44 100 Hz); the audio sink inlines its own calculation using
/// `VisualizationSink::sample_rate` so both sides are independently accurate.
pub fn decay_for_elapsed(elapsed: std::time::Duration) -> f32 {
    let elapsed_hops = elapsed.as_secs_f32() * SAMPLE_RATE / HOP_SIZE as f32;
    DECAY_FACTOR.powf(elapsed_hops)
}

/// Returns the compound `DECAY_FACTOR_PEAK` multiplier for the given elapsed
/// wall-clock duration.
///
/// Used **only on the render side** to decay the peak-envelope estimate between
/// audio packets. See `decay_for_elapsed` for the render-vs-sink split.
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
    /// Shared state written every hop and read by the UI render thread.
    /// Guarded by a `Mutex`; the render path uses `try_lock()` to avoid
    /// blocking the audio thread.
    bands: Arc<Mutex<VisBands>>,
    /// Forward FFT plan reused every hop — `rustfft` plans are thread-safe
    /// and allocation-free once created.
    fft: Arc<dyn rustfft::Fft<f32>>,
    /// Precomputed Hann window coefficients — computed once in `new()` and
    /// reused every hop to avoid repeated `cos()` calls in the hot path.
    hann_window: Vec<f32>,
    /// Reusable FFT input buffer — avoids a `Vec` allocation per hop.
    fft_buf: Vec<Complex<f32>>,
    /// Reusable magnitude buffer — avoids a `Vec` allocation per hop.
    magnitudes: Vec<f32>,
    /// Actual audio sample rate in Hz — used for precise hop-based decay
    /// calculation, since librespot can run at 44100 or 48000 Hz.
    sample_rate: f32,
    /// Precomputed (start, end) bin-index ranges for each log-scale band.
    /// Computed once in `new()` so `write()` never runs `powf` per hop.
    band_ranges: Vec<(usize, usize)>,
    /// Reusable output buffer for `fill_log_bands` — no `Vec` allocation per hop.
    new_bands: [f32; NUM_BANDS],
    /// Scratch buffer for `smooth_bands` — avoids `to_vec()` allocation per hop.
    smooth_scratch: [f32; NUM_BANDS],
}

impl VisualizationSink {
    /// Create a new `VisualizationSink` wrapping `inner`.
    ///
    /// `sample_rate` should match the actual librespot audio format sample rate
    /// (44100 or 48000 Hz) so that hop-based decay timings are accurate.
    pub fn new(inner: Box<dyn Sink>, bands: Arc<Mutex<VisBands>>, sample_rate: f32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let hann_window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
            })
            .collect();
        let band_ranges = precompute_band_ranges(FFT_SIZE / 2, NUM_BANDS);
        Self {
            inner,
            sample_buf: VecDeque::with_capacity(FFT_SIZE * 2),
            bands,
            fft,
            hann_window,
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            magnitudes: vec![0.0; FFT_SIZE / 2],
            sample_rate,
            band_ranges,
            new_bands: [0.0f32; NUM_BANDS],
            smooth_scratch: [0.0f32; NUM_BANDS],
        }
    }
}

impl Sink for VisualizationSink {
    fn start(&mut self) -> SinkResult<()> {
        self.inner.start()
    }

    fn stop(&mut self) -> SinkResult<()> {
        // Zero out the bands and reset normalization when playback stops so the
        // bars fall to silence and the next session starts with a fresh baseline.
        let mut g = self.bands.lock();
        g.values.fill(0.0);
        g.peak_envelope = 1e-6;
        g.updated_at = Instant::now();
        g.is_active = false;
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
            // vis_bands.updated_at: hops within the same write() call are ~3 ms apart,
            // so DECAY_FACTOR ^ (~1 hop elapsed) ≈ 0.985 ≈ 1.0 — no peak smearing.
            while self.sample_buf.len() >= FFT_SIZE {
                // Fill fft_buf using as_slices() to avoid make_contiguous()'s
                // potential O(n) rotation, and reuse the preallocated buffer.
                {
                    let (front, back) = self.sample_buf.as_slices();
                    if front.len() >= FFT_SIZE {
                        for (dst, (&s, &w)) in self
                            .fft_buf
                            .iter_mut()
                            .zip(front.iter().zip(self.hann_window.iter()))
                        {
                            *dst = Complex::new(s * w, 0.0);
                        }
                    } else {
                        let split = front.len();
                        for (dst, (&s, &w)) in self.fft_buf[..split]
                            .iter_mut()
                            .zip(front.iter().zip(self.hann_window[..split].iter()))
                        {
                            *dst = Complex::new(s * w, 0.0);
                        }
                        let remaining = FFT_SIZE - split;
                        for (dst, (&s, &w)) in self.fft_buf[split..].iter_mut().zip(
                            back[..remaining]
                                .iter()
                                .zip(self.hann_window[split..].iter()),
                        ) {
                            *dst = Complex::new(s * w, 0.0);
                        }
                    }
                }

                self.fft.process(&mut self.fft_buf);

                // Compute magnitudes in place — no allocation per hop.
                for (mag, c) in self.magnitudes.iter_mut().zip(self.fft_buf.iter()) {
                    *mag = c.norm();
                }

                // Fill pre-allocated band buffers in-place — no Vec allocation per hop.
                fill_log_bands(&self.magnitudes, &self.band_ranges, &mut self.new_bands);
                smooth_bands(&mut self.new_bands, &mut self.smooth_scratch);

                // Apply wall-clock decay since the last hop, then rise to any louder value.
                // Use self.sample_rate for precision (may be 44100 or 48000 Hz).
                let mut g = self.bands.lock();
                let elapsed_hops =
                    g.updated_at.elapsed().as_secs_f32() * self.sample_rate / HOP_SIZE as f32;
                let decay = DECAY_FACTOR.powf(elapsed_hops);
                let peak_decay = DECAY_FACTOR_PEAK.powf(elapsed_hops);
                let frame_peak = self.new_bands.iter().copied().fold(0.0_f32, f32::max);
                for (stored, fresh) in g.values.iter_mut().zip(self.new_bands.iter()) {
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

/// Precomputes the `(start, end)` FFT bin ranges for each log-scale band.
///
/// Called once in `VisualizationSink::new()`; the result is stored and
/// reused every hop so `write()` never runs `powf` per band per frame.
/// Bin 0 (DC component) is skipped by starting `used_up_to` at 1.
fn precompute_band_ranges(num_bins: usize, num_bands: usize) -> Vec<(usize, usize)> {
    let log_min = 1.0_f64;
    let log_max = num_bins as f64;
    let mut used_up_to: usize = 1;
    let mut ranges = Vec::with_capacity(num_bands);
    for band in 0..num_bands {
        if used_up_to >= num_bins {
            // All bins exhausted — pad remaining bands with a silent dummy range.
            ranges.push((num_bins - 1, num_bins));
            continue;
        }
        let t_start = band as f64 / num_bands as f64;
        let t_end = (band + 1) as f64 / num_bands as f64;
        let natural_start = (log_min * (log_max / log_min).powf(t_start)) as usize;
        let natural_end = (log_min * (log_max / log_min).powf(t_end)) as usize;
        // Advance past already-used bins so low-frequency bands do not all
        // share the same FFT bin and produce an identical flat plateau.
        let start = natural_start.max(used_up_to).min(num_bins - 1);
        let end = natural_end.max(start + 1).min(num_bins);
        used_up_to = end;
        ranges.push((start, end));
    }
    ranges
}

/// Fills `out` with the RMS magnitude of each log-scale band using the
/// precomputed bin ranges — no `Vec` allocation and no `powf` per call.
fn fill_log_bands(magnitudes: &[f32], band_ranges: &[(usize, usize)], out: &mut [f32]) {
    for (band_val, &(start, end)) in out.iter_mut().zip(band_ranges.iter()) {
        let len = (end - start) as f32;
        let sum_sq: f32 = magnitudes[start..end].iter().map(|&v| v * v).sum();
        *band_val = (sum_sq / len).sqrt();
    }
}

/// Applies a single pass of 3-point weighted smoothing [0.25, 0.5, 0.25] across
/// adjacent bands to reduce per-bin jitter without blurring transients.
///
/// `scratch` is a caller-supplied buffer (same length as `bands`) used as a
/// temporary copy, avoiding a `Vec` allocation on every hop.
fn smooth_bands(bands: &mut [f32], scratch: &mut [f32]) {
    let n = bands.len();
    if n < 3 {
        return;
    }
    scratch[..n].copy_from_slice(&bands[..n]);
    for i in 0..n {
        let prev = scratch[if i > 0 { i - 1 } else { 0 }];
        let next = scratch[if i + 1 < n { i + 1 } else { n - 1 }];
        bands[i] = prev * 0.25 + scratch[i] * 0.5 + next * 0.25;
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
    //
    // vis_bands is only Some when enable_audio_visualization is true.
    let Some(vis_lock) = state.vis_bands.as_ref() else {
        return;
    };
    let guard = vis_lock.lock();
    if !guard.is_active {
        return;
    }
    let display_decay = decay_for_elapsed(guard.updated_at.elapsed());
    let peak_norm =
        (guard.peak_envelope * peak_decay_for_elapsed(guard.updated_at.elapsed())).max(1e-6);
    // Copy the fixed-size array by value — no heap allocation.
    let values = guard.values;
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
                .text_value("")
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
