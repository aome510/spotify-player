/// UI codes to render a page.
/// A `render_*_page` function should follow (not strictly) the below steps
/// 1. get the data from the application's states
/// 2. construct the page's layout
/// 3. construct the page's widgets
/// 4. render the widgets
use super::{utils::construct_and_render_block, *};

pub fn render_search_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    // 1. Get the data
    let data = state.data.read();

    let (focus_state, current_query, input) = match ui.current_page() {
        PageState::Search {
            state,
            current_query,
            input,
        } => (state.focus, current_query, input),
        s => anyhow::bail!("expect a search page state, found {s:?}"),
    };

    let search_results = data.caches.search.get(current_query);

    // 2. Construct the page's layout
    let rect = construct_and_render_block("Search", &ui.theme, state, Borders::ALL, frame, rect);

    // search input's layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(rect);
    let search_input_rect = chunks[0];
    let rect = chunks[1];

    // track/album/artist/playlist search results layout (2x2 table)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect)
        .iter()
        .flat_map(|rect| {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(*rect)
                .to_vec()
        })
        .collect::<Vec<_>>();

    let track_rect = construct_and_render_block(
        "Tracks",
        &ui.theme,
        state,
        Borders::TOP | Borders::RIGHT,
        frame,
        chunks[0],
    );
    let album_rect =
        construct_and_render_block("Albums", &ui.theme, state, Borders::TOP, frame, chunks[1]);
    let artist_rect = construct_and_render_block(
        "Artists",
        &ui.theme,
        state,
        Borders::TOP | Borders::RIGHT,
        frame,
        chunks[2],
    );
    let playlist_rect = construct_and_render_block(
        "Playlists",
        &ui.theme,
        state,
        Borders::TOP,
        frame,
        chunks[3],
    );

    // 3. Construct the page's widgets
    let (track_list, n_tracks) = {
        let track_items = search_results
            .map(|s| {
                s.tracks
                    .iter()
                    .map(|a| {
                        (
                            format!("{} • {}", a.display_name(), a.artists_info()),
                            false,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Tracks;

        utils::construct_list_widget(&ui.theme, track_items, is_active)
    };

    let (album_list, n_albums) = {
        let album_items = search_results
            .map(|s| {
                s.albums
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Albums;

        utils::construct_list_widget(&ui.theme, album_items, is_active)
    };

    let (artist_list, n_artists) = {
        let artist_items = search_results
            .map(|s| {
                s.artists
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Artists;

        utils::construct_list_widget(&ui.theme, artist_items, is_active)
    };

    let (playlist_list, n_playlists) = {
        let playlist_items = search_results
            .map(|s| {
                s.playlists
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Playlists;

        utils::construct_list_widget(&ui.theme, playlist_items, is_active)
    };

    // 4. Render the page's widgets
    // Render the query input box
    frame.render_widget(
        Paragraph::new(input.clone()).style(
            ui.theme
                .selection_style(is_active && focus_state == SearchFocusState::Input),
        ),
        search_input_rect,
    );

    // Render the search result windows.
    // Need mutable access to the list/table states stored inside the page state for rendering.
    let page_state = match ui.current_page_mut() {
        PageState::Search { state, .. } => state,
        s => anyhow::bail!("expect a search page state, found {s:?}"),
    };
    utils::render_list_window(
        frame,
        track_list,
        track_rect,
        n_tracks,
        &mut page_state.track_list,
    );
    utils::render_list_window(
        frame,
        album_list,
        album_rect,
        n_albums,
        &mut page_state.album_list,
    );
    utils::render_list_window(
        frame,
        artist_list,
        artist_rect,
        n_artists,
        &mut page_state.artist_list,
    );
    utils::render_list_window(
        frame,
        playlist_list,
        playlist_rect,
        n_playlists,
        &mut page_state.playlist_list,
    );

    Ok(())
}

pub fn render_context_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    // 1. Get the data
    let (id, context_page_type) = match ui.current_page() {
        PageState::Context {
            id,
            context_page_type,
            ..
        } => (id, context_page_type),
        s => anyhow::bail!("expect a context page state, found {s:?}"),
    };

    // 2. Construct the page's layout
    let rect = construct_and_render_block(
        &context_page_type.title(),
        &ui.theme,
        state,
        Borders::ALL,
        frame,
        rect,
    );

    // 3+4. Construct and render the page's widgets
    let id = match id {
        None => {
            frame.render_widget(
                Paragraph::new("Cannot determine the current page's context"),
                rect,
            );
            return Ok(());
        }
        Some(id) => id,
    };

    let data = state.data.read();
    match data.caches.context.get(&id.uri()) {
        Some(context) => {
            // render context description
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);
            frame.render_widget(
                Paragraph::new(Text::styled(context.description(), ui.theme.page_desc())),
                chunks[0],
            );

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
                        &data,
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
                        &data,
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
                        &data,
                    )?;
                }
                Context::Tracks { tracks, .. } => {
                    render_track_table_window(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        ui.search_filtered_items(tracks),
                        ui,
                        &data,
                    )?;
                }
            }
        }
        None => {
            frame.render_widget(Paragraph::new("Loading..."), rect);
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
    // 1. Get the data
    let curr_context_uri = state.player.read().playing_context_id().map(|c| c.uri());
    let data = state.data.read();

    let focus_state = match ui.current_page() {
        PageState::Library { state } => state.focus,
        s => anyhow::bail!("expect a library page state, found {s:?}"),
    };

    // 2. Construct the page's layout
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
    let playlist_rect = construct_and_render_block(
        "Playlists",
        &ui.theme,
        state,
        Borders::TOP | Borders::LEFT | Borders::BOTTOM,
        frame,
        chunks[0],
    );
    let album_rect = construct_and_render_block(
        "Albums",
        &ui.theme,
        state,
        Borders::TOP | Borders::LEFT | Borders::BOTTOM,
        frame,
        chunks[1],
    );
    let artist_rect =
        construct_and_render_block("Artists", &ui.theme, state, Borders::ALL, frame, chunks[2]);

    // 3. Construct the page's widgets
    // Construct the playlist window
    let (playlist_list, n_playlists) = utils::construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.playlists)
            .into_iter()
            .map(|p| (p.to_string(), curr_context_uri == Some(p.id.uri())))
            .collect(),
        is_active && focus_state == LibraryFocusState::Playlists,
    );
    // Construct the saved album window
    let (album_list, n_albums) = utils::construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.saved_albums)
            .into_iter()
            .map(|a| (a.to_string(), curr_context_uri == Some(a.id.uri())))
            .collect(),
        is_active && focus_state == LibraryFocusState::SavedAlbums,
    );
    // Construct the followed artist window
    let (artist_list, n_artists) = utils::construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.followed_artists)
            .into_iter()
            .map(|a| (a.to_string(), curr_context_uri == Some(a.id.uri())))
            .collect(),
        is_active && focus_state == LibraryFocusState::FollowedArtists,
    );

    // 4. Render the page's widgets
    // Render the library page's windows.
    // Will need mutable access to the list/table states stored inside the page state for rendering.
    let page_state = match ui.current_page_mut() {
        PageState::Library { state } => state,
        s => anyhow::bail!("expect a library page state, found {s:?}"),
    };

    utils::render_list_window(
        frame,
        playlist_list,
        playlist_rect,
        n_playlists,
        &mut page_state.playlist_list,
    );
    utils::render_list_window(
        frame,
        album_list,
        album_rect,
        n_albums,
        &mut page_state.saved_album_list,
    );
    utils::render_list_window(
        frame,
        artist_list,
        artist_rect,
        n_artists,
        &mut page_state.followed_artist_list,
    );

    Ok(())
}

pub fn render_browse_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    mut rect: Rect,
) -> Result<()> {
    // 1. Get the data
    let data = state.data.read();

    // 2+3. Construct the page's layout and widgets
    let (list, len) = match ui.current_page() {
        PageState::Browse { state: ui_state } => match ui_state {
            BrowsePageUIState::CategoryList { .. } => {
                rect = construct_and_render_block(
                    "Categories",
                    &ui.theme,
                    state,
                    Borders::ALL,
                    frame,
                    rect,
                );

                utils::construct_list_widget(
                    &ui.theme,
                    ui.search_filtered_items(&data.browse.categories)
                        .into_iter()
                        .map(|c| (c.name.clone(), false))
                        .collect(),
                    is_active,
                )
            }
            BrowsePageUIState::CategoryPlaylistList { category, .. } => {
                let title = format!("{} Playlists", category.name);
                let playlists = match data.browse.category_playlists.get(&category.id) {
                    Some(playlists) => playlists,
                    None => {
                        utils::render_loading_window(state, &ui.theme, frame, rect, &title);
                        return Ok(());
                    }
                };

                rect =
                    construct_and_render_block(&title, &ui.theme, state, Borders::ALL, frame, rect);

                utils::construct_list_widget(
                    &ui.theme,
                    ui.search_filtered_items(playlists)
                        .into_iter()
                        .map(|c| (c.name.clone(), false))
                        .collect(),
                    is_active,
                )
            }
        },
        s => anyhow::bail!("expect a browse page state, found {s:?}"),
    };

    // 4. Render the page's widgets
    let list_state = match ui.current_page_mut().focus_window_state_mut() {
        Some(MutableWindowState::List(list_state)) => list_state,
        _ => anyhow::bail!("expect a list for the focused window"),
    };

    utils::render_list_window(frame, list, rect, len, list_state);

    Ok(())
}

#[cfg(feature = "lyric-finder")]
pub fn render_lyric_page(
    _is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    // 1. Get the data
    let data = state.data.read();

    // 2. Construct the app's layout
    let rect = construct_and_render_block("Lyric", &ui.theme, state, Borders::ALL, frame, rect);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(rect);

    // 3. Construct the app's widgets
    let (track, artists, scroll_offset) = match ui.current_page_mut() {
        PageState::Lyric {
            track,
            artists,
            scroll_offset,
        } => (track, artists, scroll_offset),
        s => anyhow::bail!("expect a lyric page state, found {s:?}"),
    };

    let (desc, lyric) = match data.caches.lyrics.get(&format!("{track} {artists}")) {
        None => {
            frame.render_widget(Paragraph::new("Loading..."), rect);
            return Ok(());
        }
        Some(lyric_finder::LyricResult::None) => {
            frame.render_widget(Paragraph::new("Lyric not found"), rect);
            return Ok(());
        }
        Some(lyric_finder::LyricResult::Some {
            track,
            artists,
            lyric,
        }) => (format!("{track} by {artists}"), format!("\n{lyric}")),
    };

    // update the scroll offset so that it doesn't exceed the lyric's length
    let n_rows = lyric.matches('\n').count() + 1;
    if *scroll_offset >= n_rows {
        *scroll_offset = n_rows - 1;
    }
    let scroll_offset = *scroll_offset;

    // 4. Render the app's widgets
    // render lyric page description text
    frame.render_widget(
        Paragraph::new(Text::styled(desc, ui.theme.page_desc())),
        chunks[0],
    );

    // render lyric text
    frame.render_widget(
        Paragraph::new(lyric).scroll((scroll_offset as u16, 0)),
        chunks[1],
    );

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
    data: &DataReadGuard,
    rect: Rect,
    artist_data: (&[Track], &[Album], &[Artist]),
) -> Result<()> {
    // 1. Get the data
    let (tracks, albums, artists) = (
        ui.search_filtered_items(artist_data.0),
        ui.search_filtered_items(artist_data.1),
        ui.search_filtered_items(artist_data.2),
    );

    let focus_state = match ui.current_page() {
        PageState::Context {
            state: Some(ContextPageUIState::Artist { focus, .. }),
            ..
        } => *focus,
        s => anyhow::bail!("expect an artist context page state, found {s:?}"),
    };

    // 2. Construct the app's layout
    // top tracks window
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
        .split(rect);
    let top_tracks_rect = chunks[0];

    // albums and related artitsts windows
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);
    let albums_rect = construct_and_render_block(
        "Albums",
        &ui.theme,
        state,
        Borders::TOP | Borders::RIGHT,
        frame,
        chunks[0],
    );
    let related_artists_rect = construct_and_render_block(
        "Related Artists",
        &ui.theme,
        state,
        Borders::TOP,
        frame,
        chunks[1],
    );

    // 3. Construct the widgets
    // album list widget
    let (album_list, n_albums) = {
        let album_items = albums
            .into_iter()
            .map(|a| (format!("{1} • {0}", a.name, a.year()), false))
            .collect::<Vec<_>>();

        utils::construct_list_widget(
            &ui.theme,
            album_items,
            is_active && focus_state == ArtistFocusState::Albums,
        )
    };

    // artist list widget
    let (artist_list, n_artists) = {
        let artist_items = artists
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        utils::construct_list_widget(
            &ui.theme,
            artist_items,
            is_active && focus_state == ArtistFocusState::RelatedArtists,
        )
    };

    // 4. Render the widgets
    render_track_table_window(
        frame,
        top_tracks_rect,
        is_active && focus_state == ArtistFocusState::TopTracks,
        state,
        tracks,
        ui,
        data,
    )?;

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

    utils::render_list_window(frame, album_list, albums_rect, n_albums, album_list_state);
    utils::render_list_window(
        frame,
        artist_list,
        related_artists_rect,
        n_artists,
        artist_list_state,
    );

    Ok(())
}

pub fn render_track_table_window(
    frame: &mut Frame,
    rect: Rect,
    is_active: bool,
    state: &SharedState,
    tracks: Vec<&Track>,
    ui: &mut UIStateGuard,
    data: &DataReadGuard,
) -> Result<()> {
    // get the current playing track's URI to decorate such track (if exists) in the track table
    let mut playing_track_uri = "".to_string();
    let mut playing_id = "";
    if let Some(ref playback) = state.player.read().playback {
        if let Some(rspotify_model::PlayableItem::Track(ref track)) = playback.item {
            playing_track_uri = track.id.as_ref().map(|id| id.uri()).unwrap_or_default();

            playing_id = if playback.is_playing {
                &state.configs.app_config.play_icon
            } else {
                &state.configs.app_config.pause_icon
            };
        }
    }

    let n_tracks = tracks.len();
    let rows = tracks
        .into_iter()
        .enumerate()
        .map(|(id, t)| {
            let (id, style) = if playing_track_uri == t.id.uri() {
                (playing_id.to_string(), ui.theme.current_playing())
            } else {
                ((id + 1).to_string(), Style::default())
            };
            Row::new(vec![
                Cell::from(if data.user_data.is_liked_track(t) {
                    &state.configs.app_config.liked_icon
                } else {
                    ""
                }),
                Cell::from(id),
                Cell::from(t.display_name()),
                Cell::from(t.artists_info()),
                Cell::from(t.album_info()),
                Cell::from(format!(
                    "{}:{:02}",
                    t.duration.as_secs() / 60,
                    t.duration.as_secs() % 60,
                )),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();

    let track_table = Table::new(rows)
        .header(
            Row::new(vec![
                Cell::from(""),
                Cell::from("#"),
                Cell::from("Title"),
                Cell::from("Artists"),
                Cell::from("Album"),
                Cell::from("Duration"),
            ])
            .style(ui.theme.table_header()),
        )
        .widths(&[
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ])
        .column_spacing(2)
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
                ContextPageUIState::Tracks { track_table } => track_table,
            };
            utils::render_table_window(frame, track_table, rect, n_tracks, track_table_state);
        }
        s => anyhow::bail!("reach unsupported page state {s:?} when rendering track table"),
    }

    Ok(())
}
