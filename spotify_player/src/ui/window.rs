use super::{render_track_table_widget, Frame};
use crate::{state::*, ui::construct_list_widget, utils};
use tui::{layout::*, style::*, text::*, widgets::*};

/// renders the search window showing the search results
/// of Spotify items (tracks, artists, albums, playlists) that match a given query
///
/// # Panic
/// This function will panic if the current UI's `PageState` is not `PageState::Searching`
pub fn render_search_window(is_active: bool, frame: &mut Frame, state: &SharedState, rect: Rect) {
    // gets the current search query from UI's `PageState`
    let (input, current_query) = match state.ui.lock().current_page() {
        PageState::Searching {
            input,
            current_query,
        } => (input.clone(), current_query.clone()),
        _ => unreachable!(),
    };

    let focus_state = match state.ui.lock().window {
        WindowState::Search { focus, .. } => focus,
        _ => {
            return;
        }
    };

    let data = state.data.read();

    let search_results = data.caches.search.peek(&current_query);

    let track_list = {
        let track_items = search_results
            .map(|s| {
                s.tracks
                    .iter()
                    .map(|a| (format!("{} - {}", a.name, a.artists_info()), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Tracks;

        construct_list_widget(
            state,
            track_items,
            &format!("Tracks{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
    };

    let album_list = {
        let album_items = search_results
            .map(|s| {
                s.albums
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Albums;

        construct_list_widget(
            state,
            album_items,
            &format!("Albums{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
    };

    let artist_list = {
        let artist_items = search_results
            .map(|s| {
                s.artists
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Artists;

        construct_list_widget(
            state,
            artist_items,
            &format!("Artists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
    };

    let playlist_list = {
        let playlist_items = search_results
            .map(|s| {
                s.playlists
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Playlists;

        construct_list_widget(
            state,
            playlist_items,
            &format!("Playlists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
    };

    // renders borders with title
    let block = Block::default()
        .title(state.ui.lock().theme.block_title_with_style("Search"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    // renders the query input box
    let rect = {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
            .split(rect);

        let is_active = is_active && focus_state == SearchFocusState::Input;

        frame.render_widget(
            Paragraph::new(input).style(state.ui.lock().theme.selection_style(is_active)),
            chunks[0],
        );

        chunks[1]
    };

    // split the given `rect` layout into a 2x2 layout consiting of 4 chunks
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect)
        .into_iter()
        .flat_map(|rect| {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(rect)
        })
        .collect::<Vec<_>>();

    // get the mutable list states inside the UI's `WindowState`
    // to render the search window's sub-windows
    let mut ui = state.ui.lock();
    let (track_list_state, album_list_state, artist_list_state, playlist_list_state) =
        match ui.window {
            WindowState::Search {
                ref mut track_list,
                ref mut album_list,
                ref mut artist_list,
                ref mut playlist_list,
                ..
            } => (track_list, album_list, artist_list, playlist_list),
            _ => unreachable!(),
        };

    frame.render_stateful_widget(track_list, chunks[0], track_list_state);
    frame.render_stateful_widget(album_list, chunks[1], album_list_state);
    frame.render_stateful_widget(artist_list, chunks[2], artist_list_state);
    frame.render_stateful_widget(playlist_list, chunks[3], playlist_list_state);
}

/// renders the context window which can be
/// - Current Playing: display the playing context of the current track
/// - Browsing: display the context of an arbitrary context
pub fn render_context_window(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    rect: Rect,
    title: &str,
) {
    let block = Block::default()
        .title(state.ui.lock().theme.block_title_with_style(title))
        .borders(Borders::ALL);

    let data = state.data.read();
    let context = state.player.read().context(&data.caches);

    match context {
        Some(context) => {
            frame.render_widget(block, rect);

            // render context description
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);
            let context_desc = Paragraph::new(context.description())
                .block(Block::default().style(state.ui.lock().theme.context_desc()));
            frame.render_widget(context_desc, chunks[0]);

            match context {
                Context::Artist {
                    top_tracks,
                    albums,
                    related_artists,
                    ..
                } => {
                    render_context_artist_widgets(
                        is_active,
                        frame,
                        state,
                        chunks[1],
                        (top_tracks, albums, related_artists),
                    );
                }
                Context::Playlist { tracks, .. } => {
                    render_track_table_widget(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        state.filtered_items_by_search(tracks),
                    );
                }
                Context::Album { tracks, .. } => {
                    render_track_table_widget(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        state.filtered_items_by_search(tracks),
                    );
                }
            }
        }
        None => {
            let desc = if state.player.read().context_id.is_none() {
                "Cannot infer the playing context from the current playback"
            } else {
                // context is not empty, but cannot get context data inside the player state
                // => still loading the context data
                "Loading..."
            };
            frame.render_widget(Paragraph::new(desc).block(block), rect);
        }
    }
}

/// renders the recommendation window
pub fn render_recommendation_window(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    rect: Rect,
) {
    let seed = match state.ui.lock().current_page() {
        PageState::Recommendations(seed) => seed.clone(),
        _ => unreachable!(),
    };

    let block = Block::default()
        .title(
            state
                .ui
                .lock()
                .theme
                .block_title_with_style("Recommendations"),
        )
        .borders(Borders::ALL);

    let data = state.data.read();

    let tracks = match data.caches.recommendation.peek(&seed.uri()) {
        Some(tracks) => tracks,
        None => {
            // recommendation tracks are still loading
            frame.render_widget(Paragraph::new("loading...").block(block), rect);
            return;
        }
    };

    // render the window's border and title
    frame.render_widget(block, rect);

    // render the window's description
    let desc = match seed {
        SeedItem::Track(track) => format!("{} Radio", track.name),
        SeedItem::Artist(artist) => format!("{} Radio", artist.name),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(rect);
    let context_desc =
        Paragraph::new(desc).block(Block::default().style(state.ui.lock().theme.context_desc()));
    frame.render_widget(context_desc, chunks[0]);

    render_track_table_widget(
        frame,
        chunks[1],
        is_active,
        state,
        state.filtered_items_by_search(tracks),
    );
}

/// renders a playback window showing information about the current playback such as
/// - track title, artists, album
/// - playback metadata (playing state, repeat state, shuffle state, volume, device, etc)
pub fn render_playback_window(frame: &mut Frame, state: &SharedState, rect: Rect) {
    let mut ui = state.ui.lock();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .margin(1)
        .split(rect);

    let block = Block::default()
        .title(ui.theme.block_title_with_style("Playback"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    let player = state.player.read();
    if let Some(ref playback) = player.playback {
        if let Some(rspotify::model::PlayableItem::Track(ref track)) = playback.item {
            let playback_info = vec![
                Span::styled(
                    format!(
                        "{}  {} by {}",
                        if !playback.is_playing { "⏸" } else { "▶" },
                        track.name,
                        track
                            .artists
                            .iter()
                            .map(|a| a.name.clone())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    ui.theme.playback_track(),
                )
                .into(),
                Span::styled(track.album.name.to_string(), ui.theme.playback_album()).into(),
                Span::styled(
                    format!(
                        "repeat: {} | shuffle: {} | volume: {}% | device: {}",
                        playback.repeat_state.as_ref(),
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
                // .style(theme.text_desc_style())
                .block(Block::default());
            let progress = std::cmp::min(player.playback_progress().unwrap(), track.duration);
            let progress_bar = Gauge::default()
                .block(Block::default())
                .gauge_style(ui.theme.playback_progress_bar())
                .ratio(progress.as_secs_f64() / track.duration.as_secs_f64())
                .label(Span::styled(
                    format!(
                        "{}/{}",
                        utils::format_duration(progress),
                        utils::format_duration(track.duration),
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                ));

            ui.progress_bar_rect = chunks[1];

            frame.render_widget(playback_desc, chunks[0]);
            frame.render_widget(progress_bar, chunks[1]);
        }
    };
}

/// renders the widgets for the artist context window, which includes
/// - A top track table
/// - An album list
/// - A related artist list
fn render_context_artist_widgets(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    rect: Rect,
    data: (&[Track], &[Album], &[Artist]),
) {
    let (tracks, albums, artists) = (
        state.filtered_items_by_search(data.0),
        state.filtered_items_by_search(data.1),
        state.filtered_items_by_search(data.2),
    );

    let focus_state = match state.ui.lock().window {
        WindowState::Artist { focus, .. } => focus,
        _ => {
            return;
        }
    };

    let rect = {
        // render the top tracks table for artist context window

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
            .split(rect);

        render_track_table_widget(
            frame,
            chunks[0],
            is_active && focus_state == ArtistFocusState::TopTracks,
            state,
            tracks,
        );

        chunks[1]
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect);

    // construct album list widget
    let album_list = {
        let album_items = albums
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            state,
            album_items,
            "Albums",
            is_active && focus_state == ArtistFocusState::Albums,
            Some(Borders::TOP),
        )
    };

    // construct artist list widget
    let artist_list = {
        let artist_items = artists
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            state,
            artist_items,
            "Related Artists",
            is_active && focus_state == ArtistFocusState::RelatedArtists,
            Some(Borders::TOP | Borders::LEFT),
        )
    };

    let mut ui = state.ui.lock();
    let (album_list_state, artist_list_state) = match ui.window {
        WindowState::Artist {
            ref mut album_list,
            ref mut related_artist_list,
            ..
        } => (album_list, related_artist_list),
        _ => unreachable!(),
    };

    frame.render_stateful_widget(album_list, chunks[0], album_list_state);
    frame.render_stateful_widget(artist_list, chunks[1], artist_list_state);
}
