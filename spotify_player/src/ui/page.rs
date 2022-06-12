use super::*;

pub fn render_search_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    let data = state.data.read();

    let (focus_state, current_query, input) = match ui.current_page() {
        PageState::Search {
            state,
            current_query,
            input,
        } => (state.focus, current_query, input),
        s => anyhow::bail!("expect a search page state, found {s:?}"),
    };

    let search_results = data.caches.search.peek(current_query);

    let track_list = {
        let track_items = search_results
            .map(|s| {
                s.tracks
                    .iter()
                    .map(|a| (format!("{} • {}", a.name, a.artists_info()), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Tracks;

        construct_list_widget(
            &ui.theme,
            track_items,
            &format!("Tracks{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
        .0
    };

    let album_list = {
        let album_items = search_results
            .map(|s| {
                s.albums
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Albums;

        construct_list_widget(
            &ui.theme,
            album_items,
            &format!("Albums{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
        .0
    };

    let artist_list = {
        let artist_items = search_results
            .map(|s| {
                s.artists
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Artists;

        construct_list_widget(
            &ui.theme,
            artist_items,
            &format!("Artists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
        .0
    };

    let playlist_list = {
        let playlist_items = search_results
            .map(|s| {
                s.playlists
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Playlists;

        construct_list_widget(
            &ui.theme,
            playlist_items,
            &format!("Playlists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
        .0
    };

    // renders borders with title
    let block = Block::default()
        .title(ui.theme.block_title_with_style("Search"))
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
            Paragraph::new(input.clone()).style(ui.theme.selection_style(is_active)),
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

    // Render the search page's windows.
    // Will need mutable access to the list/table states stored inside the page state for rendering.
    let page_state = match ui.current_page_mut() {
        PageState::Search { state, .. } => state,
        s => anyhow::bail!("expect a search page state, found {s:?}"),
    };
    frame.render_stateful_widget(track_list, chunks[0], &mut page_state.track_list);
    frame.render_stateful_widget(album_list, chunks[1], &mut page_state.album_list);
    frame.render_stateful_widget(artist_list, chunks[2], &mut page_state.artist_list);
    frame.render_stateful_widget(playlist_list, chunks[3], &mut page_state.playlist_list);

    Ok(())
}

pub fn render_context_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    let (id, context_page_type) = match ui.current_page() {
        PageState::Context {
            id,
            context_page_type,
            ..
        } => (id, context_page_type),
        s => anyhow::bail!("expect a context page state, found {s:?}"),
    };

    let block = Block::default()
        .title(ui.theme.block_title_with_style(match context_page_type {
            ContextPageType::CurrentPlaying => "Context (Current Playing)",
            ContextPageType::Browsing(_) => "Context (Browsing)",
        }))
        .borders(Borders::ALL);

    let context_uri = match id {
        None => {
            frame.render_widget(
                Paragraph::new("Cannot determine the current page's context").block(block),
                rect,
            );
            return Ok(());
        }
        Some(id) => id.uri(),
    };

    match state.data.read().caches.context.peek(&context_uri) {
        Some(context) => {
            frame.render_widget(block, rect);

            // render context description
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);
            let page_desc = Paragraph::new(context.description())
                .block(Block::default().style(ui.theme.page_desc()));
            frame.render_widget(page_desc, chunks[0]);

            match context {
                Context::Artist {
                    top_tracks,
                    albums,
                    related_artists,
                    ..
                } => {
                    render_artist_context_page_windows(
                        is_active,
                        frame,
                        state,
                        ui,
                        chunks[1],
                        (top_tracks, albums, related_artists),
                    )?;
                }
                Context::Playlist { tracks, .. } => {
                    render_track_table_window(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        ui.search_filtered_items(tracks),
                        ui,
                    )?;
                }
                Context::Album { tracks, .. } => {
                    render_track_table_window(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        ui.search_filtered_items(tracks),
                        ui,
                    )?;
                }
            }
        }
        None => {
            frame.render_widget(Paragraph::new("Loading...").block(block), rect);
        }
    }

    Ok(())
}

pub fn render_library_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    let curr_context_uri = state.player.read().playing_context_id().map(|c| c.uri());
    let data = state.data.read();

    let focus_state = match ui.current_page() {
        PageState::Library { state } => state.focus,
        s => anyhow::bail!("expect a library page state, found {s:?}"),
    };

    // Horizontally split the library page into 3 windows:
    // - a playlists window
    // - a saved albums window
    // - a followed artists window
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(40),
                Constraint::Percentage(40),
                Constraint::Percentage(20),
            ]
            .as_ref(),
        )
        .split(rect);
    let (playlist_rect, album_rect, artist_rect) = (chunks[0], chunks[1], chunks[2]);

    // Construct the playlist window
    let (playlist_list, n_playlists) = construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.playlists)
            .into_iter()
            .map(|p| (p.to_string(), curr_context_uri == Some(p.id.uri())))
            .collect(),
        "Playlists",
        is_active && focus_state == LibraryFocusState::Playlists,
        Some((Borders::TOP | Borders::LEFT) | Borders::BOTTOM),
    );
    // Construct the saved album window
    let (album_list, n_albums) = construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.saved_albums)
            .into_iter()
            .map(|a| (a.to_string(), curr_context_uri == Some(a.id.uri())))
            .collect(),
        "Albums",
        is_active && focus_state == LibraryFocusState::SavedAlbums,
        Some((Borders::TOP | Borders::LEFT) | Borders::BOTTOM),
    );
    // Construct the followed artist window
    let (artist_list, n_artists) = construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.followed_artists)
            .into_iter()
            .map(|a| (a.to_string(), curr_context_uri == Some(a.id.uri())))
            .collect(),
        "Artists",
        is_active && focus_state == LibraryFocusState::FollowedArtists,
        None,
    );

    // Render the library page's windows.
    // Will need mutable access to the list/table states stored inside the page state for rendering.
    let page_state = match ui.current_page_mut() {
        PageState::Library { state } => state,
        s => anyhow::bail!("expect a library page state, found {s:?}"),
    };

    // adjust the `selected` position of a `ListState` if that position is out of index
    let adjust_list_state = |state: &mut ListState, len: usize| {
        if let Some(p) = state.selected() {
            if p >= len {
                state.select(if len > 0 { Some(len - 1) } else { Some(0) });
            }
        }
    };
    adjust_list_state(&mut page_state.playlist_list, n_playlists);
    adjust_list_state(&mut page_state.saved_album_list, n_albums);
    adjust_list_state(&mut page_state.followed_artist_list, n_artists);

    frame.render_stateful_widget(playlist_list, playlist_rect, &mut page_state.playlist_list);
    frame.render_stateful_widget(album_list, album_rect, &mut page_state.saved_album_list);
    frame.render_stateful_widget(
        artist_list,
        artist_rect,
        &mut page_state.followed_artist_list,
    );

    Ok(())
}

pub fn render_tracks_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    let data = state.data.read();

    let (id, title, desc) = match ui.current_page() {
        PageState::Tracks {
            id, title, desc, ..
        } => (id, title, desc),
        s => anyhow::bail!("expect a tracks page state, found {s:?}"),
    };

    let block = Block::default()
        .title(ui.theme.block_title_with_style(title))
        .borders(Borders::ALL);

    let tracks = match data.caches.tracks.peek(id) {
        Some(tracks) => tracks,
        None => {
            // tracks are still loading
            frame.render_widget(Paragraph::new("loading...").block(block), rect);
            return Ok(());
        }
    };

    // render the window's border and title
    frame.render_widget(block, rect);

    // render the window's description
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(rect);
    let page_desc =
        Paragraph::new(desc.clone()).block(Block::default().style(ui.theme.page_desc()));
    frame.render_widget(page_desc, chunks[0]);

    render_track_table_window(
        frame,
        chunks[1],
        is_active,
        state,
        ui.search_filtered_items(tracks),
        ui,
    )
}

#[cfg(feature = "lyric-finder")]
pub fn render_lyric_page(
    _is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    let data = state.data.read();

    let (track, artists, scroll_offset) = match ui.current_page() {
        PageState::Lyric {
            track,
            artists,
            scroll_offset,
        } => (track, artists, *scroll_offset),
        s => anyhow::bail!("expect a lyric page state, found {s:?}"),
    };

    let block = Block::default()
        .title(ui.theme.block_title_with_style("Lyric"))
        .borders(Borders::ALL);

    let result = data.caches.lyrics.peek(&format!("{} {}", track, artists));
    match result {
        None => {
            frame.render_widget(Paragraph::new("Loading...").block(block), rect);
        }
        Some(lyric_finder::LyricResult::None) => {
            frame.render_widget(Paragraph::new("Lyric not found").block(block), rect);
        }
        Some(lyric_finder::LyricResult::Some {
            track,
            artists,
            lyric,
        }) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);

            // render lyric page borders
            frame.render_widget(block, rect);

            // render lyric page description text
            frame.render_widget(
                Paragraph::new(format!("{} by {}", track, artists))
                    .block(Block::default().style(ui.theme.page_desc())),
                chunks[0],
            );

            // render lyric text
            frame.render_widget(
                Paragraph::new(format!("\n{}", lyric))
                    .scroll((scroll_offset as u16, 0))
                    .block(Block::default()),
                chunks[1],
            );
        }
    }

    Ok(())
}

/// Renders windows for an artist context page, which includes
/// - A top track table
/// - An album list
/// - A related artist list
fn render_artist_context_page_windows(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
    data: (&[Track], &[Album], &[Artist]),
) -> Result<()> {
    let (tracks, albums, artists) = (
        ui.search_filtered_items(data.0),
        ui.search_filtered_items(data.1),
        ui.search_filtered_items(data.2),
    );

    let focus_state = match ui.current_page() {
        PageState::Context {
            state: Some(ContextPageUIState::Artist { focus, .. }),
            ..
        } => *focus,
        s => anyhow::bail!("expect an artist context page state, found {s:?}"),
    };

    let rect = {
        // render the top tracks table for artist context window

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
            .split(rect);

        render_track_table_window(
            frame,
            chunks[0],
            is_active && focus_state == ArtistFocusState::TopTracks,
            state,
            tracks,
            ui,
        )?;

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
            &ui.theme,
            album_items,
            "Albums",
            is_active && focus_state == ArtistFocusState::Albums,
            Some(Borders::TOP),
        )
        .0
    };

    // construct artist list widget
    let artist_list = {
        let artist_items = artists
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            &ui.theme,
            artist_items,
            "Related Artists",
            is_active && focus_state == ArtistFocusState::RelatedArtists,
            Some(Borders::TOP | Borders::LEFT),
        )
        .0
    };

    let (album_list_state, artist_list_state) = match ui.current_page_mut() {
        PageState::Context {
            state:
                Some(ContextPageUIState::Artist {
                    album_list,
                    related_artist_list,
                    ..
                }),
            ..
        } => (album_list, related_artist_list),
        s => anyhow::bail!("expect an artist context page state, found {s:?}"),
    };

    frame.render_stateful_widget(album_list, chunks[0], album_list_state);
    frame.render_stateful_widget(artist_list, chunks[1], artist_list_state);

    Ok(())
}

pub fn render_track_table_window(
    frame: &mut Frame,
    rect: Rect,
    is_active: bool,
    state: &SharedState,
    tracks: Vec<&Track>,
    ui: &mut UIStateGuard,
) -> Result<()> {
    // get the current playing track's URI to decorate such track (if exists) in the track table
    let mut playing_track_uri = "".to_string();
    let mut active_desc = "";
    if let Some(ref playback) = state.player.read().playback {
        if let Some(rspotify_model::PlayableItem::Track(ref track)) = playback.item {
            playing_track_uri = track.id.as_ref().map(|id| id.uri()).unwrap_or_default();

            active_desc = if !playback.is_playing { "⏸" } else { "▶" };
        }
    }

    let item_max_len = state.app_config.track_table_item_max_len;
    let rows = tracks
        .into_iter()
        .enumerate()
        .map(|(id, t)| {
            let (id, style) = if playing_track_uri == t.id.uri() {
                (active_desc.to_string(), ui.theme.current_playing())
            } else {
                ((id + 1).to_string(), Style::default())
            };
            Row::new(vec![
                Cell::from(id),
                Cell::from(utils::truncate_string(t.name.clone(), item_max_len)),
                Cell::from(utils::truncate_string(t.artists_info(), item_max_len)),
                Cell::from(utils::truncate_string(t.album_info(), item_max_len)),
                Cell::from(utils::format_duration(t.duration)),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();

    let track_table = Table::new(rows)
        .header(
            Row::new(vec![
                Cell::from("#"),
                Cell::from("Track"),
                Cell::from("Artists"),
                Cell::from("Album"),
                Cell::from("Duration"),
            ])
            .style(ui.theme.table_header()),
        )
        .block(Block::default())
        .widths(&[
            Constraint::Length(5),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
        ])
        .highlight_style(ui.theme.selection_style(is_active));

    match ui.current_page_mut() {
        PageState::Context {
            state: Some(state), ..
        } => {
            let track_table_state = match state {
                ContextPageUIState::Artist {
                    top_track_table, ..
                } => top_track_table,
                ContextPageUIState::Playlist { track_table } => track_table,
                ContextPageUIState::Album { track_table } => track_table,
            };
            frame.render_stateful_widget(track_table, rect, track_table_state);
        }
        PageState::Tracks { state, .. } => {
            frame.render_stateful_widget(track_table, rect, state);
        }
        s => anyhow::bail!("reach unsupported page state {s:?} when rendering track table"),
    }

    Ok(())
}
