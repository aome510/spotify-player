use super::*;

/// Renders a playback window showing information about the current playback, which includes
/// - track title, artists, album
/// - playback metadata (playing state, repeat state, shuffle state, volume, device, etc)
/// - cover image (if `image` feature is enabled)
/// - playback progress bar
pub fn render_playback_window(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    // render borders and title
    let block = Block::default()
        .title(ui.theme.block_title_with_style("Playback"))
        .borders(Borders::ALL)
        .border_style(ui.theme.border());
    frame.render_widget(block, rect);

    let rect = {
        // remove top/bot margins
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)].as_ref())
            .margin(1)
            .split(rect)[0]
    };

    let player = state.player.read();
    if let Some(ref playback) = player.playback {
        if let Some(rspotify::model::PlayableItem::Track(ref track)) = playback.item {
            let (metadata_rect, progress_bar_rect) = {
                // allocate the progress bar rect
                let (rect, progress_bar_rect) = {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
                        .split(rect);

                    (chunks[0], chunks[1])
                };

                let metadata_rect = {
                    // Render the track's cover image if `image` feature is enabled
                    #[cfg(feature = "image")]
                    {
                        // Split the allocated rectangle into `metadata_rect` and `cover_img_rect`
                        let (metadata_rect, cover_img_rect) = {
                            let hor_chunks = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints(
                                    [
                                        Constraint::Length(
                                            state.app_config.cover_img_length as u16,
                                        ),
                                        Constraint::Length(1), // a margin of 1 between the cover image widget and track's metadata widget
                                        Constraint::Min(0),    // metadata_rect
                                    ]
                                    .as_ref(),
                                )
                                .split(rect);
                            let ver_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints(
                                    [
                                        Constraint::Length(state.app_config.cover_img_width as u16), // cover_img_rect
                                        Constraint::Min(0), // a margin of 1 between the cover image widget and track's metadata widget
                                    ]
                                    .as_ref(),
                                )
                                .split(hor_chunks[0]);

                            (hor_chunks[2], ver_chunks[0])
                        };

                        let url = crate::utils::get_track_album_image_url(track).map(String::from);
                        if let Some(url) = url {
                            let needs_render = match &ui.last_cover_image_render_info {
                                Some((last_url, last_time)) => {
                                    url != *last_url
                                        || last_time.elapsed()
                                            > std::time::Duration::from_millis(
                                                state.app_config.cover_image_refresh_duration_in_ms,
                                            )
                                }
                                None => true,
                            };

                            if needs_render {
                                render_playback_cover_image(state, ui, cover_img_rect, url);
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

            render_playback_metadata(frame, state, ui, metadata_rect, track, playback);

            let progress = std::cmp::min(
                player
                    .playback_progress()
                    .context("playback should exist")?,
                track.duration,
            );
            render_playback_progress_bar(frame, ui, progress, track, progress_bar_rect);
        } else {
            tracing::warn!("Got a non-track playable item: {:?}", playback.item);
        }
    } else {
        // Previously rendered image can result in weird rendering text,
        // clear the widget area before rendering the text.
        #[cfg(feature = "image")]
        if ui.last_cover_image_render_info.is_some() {
            frame.render_widget(Clear, rect);
            ui.last_cover_image_render_info = None;
        }

        frame.render_widget(
            Paragraph::new(
                "No playback found. \
                 Please make sure there is a running Spotify client and try to connect to it using the `SwitchDevice` command."
            )
            .wrap(Wrap { trim: true })
            .block(Block::default()),
            rect,
        );
    };

    Ok(())
}

fn render_playback_metadata(
    frame: &mut Frame,
    state: &SharedState,
    ui: &UIStateGuard,
    rect: Rect,
    track: &rspotify_model::FullTrack,
    playback: &rspotify_model::CurrentPlaybackContext,
) {
    let playback_info = vec![
        Span::styled(
            format!(
                "{} {} • {}",
                if !playback.is_playing {
                    &state.app_config.pause_icon
                } else {
                    &state.app_config.play_icon
                },
                track.name,
                crate::utils::map_join(&track.artists, |a| &a.name, ", ")
            ),
            ui.theme.playback_track(),
        )
        .into(),
        Span::styled(track.album.name.to_string(), ui.theme.playback_album()).into(),
        Span::styled(
            format!(
                "repeat: {} | shuffle: {} | volume: {}% | device: {}",
                <&'static str>::from(playback.repeat_state),
                playback.shuffle_state,
                playback.device.volume_percent.unwrap_or_default(),
                playback.device.name,
            ),
            ui.theme.playback_metadata(),
        )
        .into(),
    ];

    let playback_desc = Paragraph::new(playback_info)
        .wrap(Wrap { trim: true })
        .block(Block::default());

    frame.render_widget(playback_desc, rect);
}

fn render_playback_progress_bar(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    progress: std::time::Duration,
    track: &rspotify_model::FullTrack,
    rect: Rect,
) {
    let progress_bar = Gauge::default()
        .block(Block::default())
        .gauge_style(ui.theme.playback_progress_bar())
        .ratio(progress.as_secs_f64() / track.duration.as_secs_f64())
        .label(Span::styled(
            format!(
                "{}/{}",
                crate::utils::format_duration(progress),
                crate::utils::format_duration(track.duration),
            ),
            Style::default().add_modifier(Modifier::BOLD),
        ));

    // update the progress bar's position stored inside the UI state
    ui.playback_progress_bar_rect = rect;

    frame.render_widget(progress_bar, rect);
}

#[cfg(feature = "image")]
fn render_playback_cover_image(
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
    url: String,
) {
    let data = state.data.read();
    if let Some(image) = data.caches.images.peek(&url) {
        ui.last_cover_image_render_info = Some((url, std::time::Instant::now()));

        let width = rect.width as u32;
        let height = rect.height as u32;

        // `viuer` renders images using `sixel` in a different scale
        // compared to other methods. Re-scale the width, length
        // to make the rendered images more fit. `1.8` seems to
        // be a "right" constant for scaling.
        // See the discussion in https://github.com/aome510/spotify-player/issues/122.
        #[cfg(feature = "sixel")]
        let width = (rect.width as f32 * 1.8) as u32;
        #[cfg(feature = "sixel")]
        let height = (rect.height as f32 * 1.8) as u32;

        if let Err(err) = viuer::print(
            image,
            &viuer::Config {
                x: rect.x,
                y: rect.y as i16,
                width: Some(width),
                height: Some(height),
                ..Default::default()
            },
        ) {
            tracing::error!("Failed to render the image: {err}",);
        }
    }
}

/// Splits the application rectangle into two rectangles, one for the playback window
/// and another for the main application's layout (popup, page, etc).
pub fn split_rect_for_playback_window(rect: Rect, state: &SharedState) -> (Rect, Rect) {
    let playback_width = state.app_config.playback_window_width;
    // the playback window's width should not be smaller than the cover image's width + 1
    #[cfg(feature = "image")]
    let playback_width = std::cmp::max(state.app_config.cover_img_width + 1, playback_width);

    // +2 for top/bottom borders
    let playback_width = (playback_width + 2) as u16;

    match state.app_config.playback_window_position {
        config::Position::Top => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(playback_width), Constraint::Min(0)].as_ref())
                .split(rect);

            (chunks[0], chunks[1])
        }
        config::Position::Bottom => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(playback_width)].as_ref())
                .split(rect);

            (chunks[1], chunks[0])
        }
    }
}
