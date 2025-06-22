#[cfg(feature = "image")]
use crate::state::ImageRenderInfo;
#[cfg(feature = "image")]
use anyhow::{Context, Result};
use rspotify::model::Id;

use super::{
    config, utils::construct_and_render_block, Borders, Constraint, Frame, Gauge, Layout, Line,
    LineGauge, Modifier, Paragraph, PlaybackMetadata, Rect, SharedState, Span, Style, Text,
    UIStateGuard, Wrap,
};

/// Render a playback window showing information about the current playback, which includes
/// - track title, artists, album
/// - playback metadata (playing state, repeat state, shuffle state, volume, device, etc)
/// - cover image (if `image` feature is enabled)
/// - playback progress bar
pub fn render_playback_window(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Rect {
    let (rect, other_rect) = split_rect_for_playback_window(rect);
    let rect = construct_and_render_block("Playback", &ui.theme, Borders::ALL, frame, rect);

    let player = state.player.read();
    if let Some(ref playback) = player.playback {
        if let Some(item) = &playback.item {
            let (metadata_rect, progress_bar_rect) = {
                // allocate the progress bar rect
                let (rect, progress_bar_rect) = {
                    let chunks =
                        Layout::vertical([Constraint::Fill(0), Constraint::Length(1)]).split(rect);

                    (chunks[0], chunks[1])
                };

                let metadata_rect = {
                    // Render the track's cover image if `image` feature is enabled
                    #[cfg(feature = "image")]
                    {
                        let configs = config::get_config();
                        // Split the allocated rectangle into `metadata_rect` and `cover_img_rect`
                        let (metadata_rect, cover_img_rect) = {
                            let hor_chunks = Layout::horizontal([
                                Constraint::Length(configs.app_config.cover_img_length as u16),
                                Constraint::Fill(0), // metadata_rect
                            ])
                            .spacing(1)
                            .split(rect);
                            let ver_chunks = Layout::vertical([
                                Constraint::Length(configs.app_config.cover_img_width as u16), // cover_img_rect
                                Constraint::Fill(0), // empty space
                            ])
                            .split(hor_chunks[0]);

                            (hor_chunks[1], ver_chunks[0])
                        };

                        let url = match item {
                            rspotify::model::PlayableItem::Track(track) => {
                                crate::utils::get_track_album_image_url(track).map(String::from)
                            }
                            rspotify::model::PlayableItem::Episode(episode) => {
                                crate::utils::get_episode_show_image_url(episode).map(String::from)
                            }
                        };
                        if let Some(url) = url {
                            let needs_clear = if ui.last_cover_image_render_info.url != url
                                || ui.last_cover_image_render_info.render_area != cover_img_rect
                            {
                                ui.last_cover_image_render_info = ImageRenderInfo {
                                    url,
                                    render_area: cover_img_rect,
                                    rendered: false,
                                };
                                true
                            } else {
                                false
                            };

                            if needs_clear {
                                // clear the image's both new and old areas to ensure no remaining artifacts before rendering the image
                                // See: https://github.com/aome510/spotify-player/issues/389
                                clear_area(
                                    frame,
                                    ui.last_cover_image_render_info.render_area,
                                    &ui.theme,
                                );
                                clear_area(frame, cover_img_rect, &ui.theme);
                            } else {
                                if !ui.last_cover_image_render_info.rendered {
                                    if let Err(err) = render_playback_cover_image(state, ui) {
                                        tracing::error!(
                                            "Failed to render playback's cover image: {err:#}"
                                        );
                                    }
                                }

                                // set the `skip` state of cells in the cover image area
                                // to prevent buffer from overwriting the image's rendered area
                                // NOTE: `skip` should not be set when clearing the render area.
                                // Otherwise, nothing will be clear as the buffer doesn't handle cells with `skip=true`.
                                for x in cover_img_rect.left()..cover_img_rect.right() {
                                    for y in cover_img_rect.top()..cover_img_rect.bottom() {
                                        frame
                                            .buffer_mut()
                                            .cell_mut((x, y))
                                            .expect("invalid cell")
                                            .set_skip(true);
                                    }
                                }
                            }
                        }

                        metadata_rect
                    }

                    #[cfg(not(feature = "image"))]
                    {
                        rect
                    }
                };

                (metadata_rect, progress_bar_rect)
            };

            if let Some(ref playback) = player.buffered_playback {
                let playback_text = construct_playback_text(ui, state, item, playback);
                let playback_desc = Paragraph::new(playback_text);
                frame.render_widget(playback_desc, metadata_rect);
            }

            let duration = match item {
                rspotify::model::PlayableItem::Track(track) => track.duration,
                rspotify::model::PlayableItem::Episode(episode) => episode.duration,
            };

            let progress = std::cmp::min(
                player.playback_progress().expect("non-empty playback"),
                duration,
            );
            render_playback_progress_bar(frame, ui, progress, duration, progress_bar_rect);
            return other_rect;
        }
    }

    // Previously rendered image can result in a weird rendering text,
    // clear the previous widget's area before rendering the text.
    #[cfg(feature = "image")]
    {
        if ui.last_cover_image_render_info.rendered {
            clear_area(
                frame,
                ui.last_cover_image_render_info.render_area,
                &ui.theme,
            );
            ui.last_cover_image_render_info = ImageRenderInfo::default();
        }
    }

    frame.render_widget(
            Paragraph::new(
                "No playback found. Please start a new playback.\n \
                 Make sure there is a running Spotify device and try to connect to one using the `SwitchDevice` command.\n \
                 You may also need to set up Spotify Connect to see available devices as in https://github.com/aome510/spotify-player#spotify-connect."
            )
            .wrap(Wrap { trim: true }),
            rect,
        );

    other_rect
}

#[cfg(feature = "image")]
fn clear_area(frame: &mut Frame, rect: Rect, theme: &config::Theme) {
    for x in rect.left()..rect.right() {
        for y in rect.top()..rect.bottom() {
            frame
                .buffer_mut()
                .cell_mut((x, y))
                .expect("invalid cell")
                .set_char(' ')
                .set_style(theme.app());
        }
    }
}

fn construct_playback_text(
    ui: &UIStateGuard,
    state: &SharedState,
    playable: &rspotify::model::PlayableItem,
    playback: &PlaybackMetadata,
) -> Text<'static> {
    // Construct a "styled" text (`playback_text`) from playback's data
    // based on a user-configurable format string (app_config.playback_format)
    let configs = config::get_config();
    let format_str = &configs.app_config.playback_format;
    let data = state.data.read();

    let mut playback_text = Text::default();
    let mut spans = vec![];

    // this regex is to handle a format argument or a newline
    let re = regex::Regex::new(r"\{.*?\}|\n").unwrap();

    let mut ptr = 0;
    for m in re.find_iter(format_str) {
        let s = m.start();
        let e = m.end();
        if ptr < s {
            spans.push(Span::raw(format_str[ptr..s].to_string()));
        }
        ptr = e;

        let (text, style) = match m.as_str() {
            // upon encountering a newline, create a new `Spans`
            "\n" => {
                let mut tmp = vec![];
                std::mem::swap(&mut tmp, &mut spans);
                playback_text.lines.push(Line::from(tmp));
                continue;
            }
            "{status}" => (
                if playback.is_playing {
                    &configs.app_config.play_icon
                } else {
                    &configs.app_config.pause_icon
                }
                .to_owned(),
                ui.theme.playback_status(),
            ),
            "{liked}" => match playable {
                rspotify::model::PlayableItem::Track(track) => match &track.id {
                    Some(id) => {
                        if data.user_data.saved_tracks.contains_key(&id.uri()) {
                            (configs.app_config.liked_icon.clone(), ui.theme.like())
                        } else {
                            continue;
                        }
                    }
                    None => continue,
                },
                rspotify::model::PlayableItem::Episode(_) => continue,
            },
            "{track}" => match playable {
                rspotify::model::PlayableItem::Track(track) => (
                    if track.explicit {
                        format!("{} (E)", track.name)
                    } else {
                        track.name.clone()
                    },
                    ui.theme.playback_track(),
                ),
                rspotify::model::PlayableItem::Episode(episode) => (
                    if episode.explicit {
                        format!("{} (E)", episode.name)
                    } else {
                        episode.name.clone()
                    },
                    ui.theme.playback_track(),
                ),
            },
            "{artists}" => match playable {
                rspotify::model::PlayableItem::Track(track) => (
                    crate::utils::map_join(&track.artists, |a| &a.name, ", "),
                    ui.theme.playback_artists(),
                ),
                rspotify::model::PlayableItem::Episode(episode) => {
                    (episode.show.publisher.clone(), ui.theme.playback_artists())
                }
            },
            "{album}" => match playable {
                rspotify::model::PlayableItem::Track(track) => {
                    (track.album.name.clone(), ui.theme.playback_album())
                }
                rspotify::model::PlayableItem::Episode(episode) => {
                    (episode.show.name.clone(), ui.theme.playback_album())
                }
            },
            "{metadata}" => (
                format!(
                    "repeat: {} | shuffle: {} | volume: {} | device: {}",
                    if playback.fake_track_repeat_state {
                        "track (fake)"
                    } else {
                        <&'static str>::from(playback.repeat_state)
                    },
                    playback.shuffle_state,
                    if let Some(volume) = playback.mute_state {
                        format!("{volume}% (muted)")
                    } else {
                        format!("{}%", playback.volume.unwrap_or_default())
                    },
                    playback.device_name,
                ),
                ui.theme.playback_metadata(),
            ),
            _ => continue,
        };

        spans.push(Span::styled(text, style));
    }
    if ptr < format_str.len() {
        spans.push(Span::raw(format_str[ptr..].to_string()));
    }
    if !spans.is_empty() {
        playback_text.lines.push(Line::from(spans));
    }

    playback_text
}

fn render_playback_progress_bar(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    progress: chrono::Duration,
    duration: chrono::Duration,
    rect: Rect,
) {
    // Negative numbers can sometimes appear from progress.num_seconds() so this stops
    // them coming through into the ratios
    let ratio = (progress.num_seconds() as f64 / duration.num_seconds() as f64).clamp(0.0, 1.0);

    match config::get_config().app_config.progress_bar_type {
        config::ProgressBarType::Line => frame.render_widget(
            LineGauge::default()
                .filled_style(ui.theme.playback_progress_bar())
                .unfilled_style(ui.theme.playback_progress_bar_unfilled())
                .ratio(ratio)
                .label(Span::styled(
                    format!(
                        "{}/{}",
                        crate::utils::format_duration(&progress),
                        crate::utils::format_duration(&duration),
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            rect,
        ),
        config::ProgressBarType::Rectangle => frame.render_widget(
            Gauge::default()
                .gauge_style(ui.theme.playback_progress_bar())
                .ratio(ratio)
                .label(Span::styled(
                    format!(
                        "{}/{}",
                        crate::utils::format_duration(&progress),
                        crate::utils::format_duration(&duration),
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            rect,
        ),
    }

    // update the progress bar's position stored inside the UI state
    ui.playback_progress_bar_rect = rect;
}

#[cfg(feature = "image")]
fn render_playback_cover_image(state: &SharedState, ui: &mut UIStateGuard) -> Result<()> {
    fn remove_temp_files() -> Result<()> {
        // Clean up temp files created by `viuer`'s kitty printer to avoid
        // possible freeze because of too many temp files in the temp folder.
        // Context: https://github.com/aome510/spotify-player/issues/148
        let tmp_dir = std::env::temp_dir();
        for path in (std::fs::read_dir(tmp_dir)?).flatten() {
            let path = path.path();
            if path.display().to_string().contains(".tmp.viuer") {
                std::fs::remove_file(path)?;
            }
        }

        Ok(())
    }

    remove_temp_files().context("remove temp files")?;

    let data = state.data.read();
    if let Some(image) = data.caches.images.get(&ui.last_cover_image_render_info.url) {
        let rect = ui.last_cover_image_render_info.render_area;

        // `viuer` renders image using `sixel` in a different scale compared to other methods.
        // Scale the image to make the rendered image more fit if needed.
        // This scaling factor is user configurable as the scale works differently
        // with different fonts and terminals.
        // For more context, see https://github.com/aome510/spotify-player/issues/122.
        let scale = config::get_config().app_config.cover_img_scale;
        let width = (f32::from(rect.width) * scale).round() as u32;
        let height = (f32::from(rect.height) * scale).round() as u32;

        viuer::print(
            image,
            &viuer::Config {
                x: rect.x,
                y: rect.y as i16,
                width: Some(width),
                height: Some(height),
                restore_cursor: true,
                transparent: true,
                ..Default::default()
            },
        )
        .context("print image to the terminal")?;

        ui.last_cover_image_render_info.rendered = true;
    }

    Ok(())
}

/// Split the given area into two, the first one for the playback window
/// and the second one for the main application's layout (popup, page, etc).
fn split_rect_for_playback_window(rect: Rect) -> (Rect, Rect) {
    let configs = config::get_config();
    let playback_width = configs.app_config.layout.playback_window_height;
    // the playback window's width should not be smaller than the cover image's width + 1
    #[cfg(feature = "image")]
    let playback_width = std::cmp::max(configs.app_config.cover_img_width + 1, playback_width);

    // +2 for top/bottom borders
    let playback_width = (playback_width + 2) as u16;

    match configs.app_config.layout.playback_window_position {
        config::Position::Top => {
            let chunks =
                Layout::vertical([Constraint::Length(playback_width), Constraint::Fill(0)])
                    .split(rect);

            (chunks[0], chunks[1])
        }
        config::Position::Bottom => {
            let chunks =
                Layout::vertical([Constraint::Fill(0), Constraint::Length(playback_width)])
                    .split(rect);

            (chunks[1], chunks[0])
        }
    }
}
