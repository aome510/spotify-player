use super::*;

// pub fn render_search_page(
//     is_active: bool,
//     frame: &mut Frame,
//     rect: Rect,
//     state: &SharedState,
//     input: &str,
//     current_query: &str,
//     page_ui_state: &mut SearchPageUIState,
// ) {
//     let data = state.data.read();

//     let search_results = data.caches.search.peek(current_query);

//     let track_list = {
//         let track_items = search_results
//             .map(|s| {
//                 s.tracks
//                     .iter()
//                     .map(|a| (format!("{} - {}", a.name, a.artists_info()), false))
//                     .collect::<Vec<_>>()
//             })
//             .unwrap_or_default();

//         let is_active = is_active && page_ui_state.focus == SearchFocusState::Tracks;

//         construct_list_widget(
//             state,
//             track_items,
//             &format!("Tracks{}", if is_active { " [*]" } else { "" }),
//             is_active,
//             Some(Borders::TOP | Borders::RIGHT),
//         )
//     };

//     let album_list = {
//         let album_items = search_results
//             .map(|s| {
//                 s.albums
//                     .iter()
//                     .map(|a| (a.name.clone(), false))
//                     .collect::<Vec<_>>()
//             })
//             .unwrap_or_default();

//         let is_active = is_active && page_ui_state.focus == SearchFocusState::Albums;

//         construct_list_widget(
//             state,
//             album_items,
//             &format!("Albums{}", if is_active { " [*]" } else { "" }),
//             is_active,
//             Some(Borders::TOP),
//         )
//     };

//     let artist_list = {
//         let artist_items = search_results
//             .map(|s| {
//                 s.artists
//                     .iter()
//                     .map(|a| (a.name.clone(), false))
//                     .collect::<Vec<_>>()
//             })
//             .unwrap_or_default();

//         let is_active = is_active && page_ui_state.focus == SearchFocusState::Artists;

//         construct_list_widget(
//             state,
//             artist_items,
//             &format!("Artists{}", if is_active { " [*]" } else { "" }),
//             is_active,
//             Some(Borders::TOP | Borders::RIGHT),
//         )
//     };

//     let playlist_list = {
//         let playlist_items = search_results
//             .map(|s| {
//                 s.playlists
//                     .iter()
//                     .map(|a| (a.name.clone(), false))
//                     .collect::<Vec<_>>()
//             })
//             .unwrap_or_default();

//         let is_active = is_active && page_ui_state.focus == SearchFocusState::Playlists;

//         construct_list_widget(
//             state,
//             playlist_items,
//             &format!("Playlists{}", if is_active { " [*]" } else { "" }),
//             is_active,
//             Some(Borders::TOP),
//         )
//     };

//     // renders borders with title
//     let block = Block::default()
//         .title(state.ui.lock().theme.block_title_with_style("Search"))
//         .borders(Borders::ALL);
//     frame.render_widget(block, rect);

//     // renders the query input box
//     let rect = {
//         let chunks = Layout::default()
//             .direction(Direction::Vertical)
//             .margin(1)
//             .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
//             .split(rect);

//         let is_active = is_active && page_ui_state.focus == SearchFocusState::Input;

//         frame.render_widget(
//             Paragraph::new(input).style(state.ui.lock().theme.selection_style(is_active)),
//             chunks[0],
//         );

//         chunks[1]
//     };

//     // split the given `rect` layout into a 2x2 layout consiting of 4 chunks
//     let chunks = Layout::default()
//         .direction(Direction::Vertical)
//         .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
//         .split(rect)
//         .into_iter()
//         .flat_map(|rect| {
//             Layout::default()
//                 .direction(Direction::Horizontal)
//                 .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
//                 .split(rect)
//         })
//         .collect::<Vec<_>>();

//     frame.render_stateful_widget(track_list, chunks[0], &mut page_ui_state.track_list);
//     frame.render_stateful_widget(album_list, chunks[1], &mut page_ui_state.album_list);
//     frame.render_stateful_widget(artist_list, chunks[2], &mut page_ui_state.artist_list);
//     frame.render_stateful_widget(playlist_list, chunks[3], &mut page_ui_state.playlist_list);
// }

// // pub fn render_context_window(is_active: bool, frame: &mut Frame, state: &SharedState, rect: Rect) {
// //     let block = Block::default()
// //         .title(state.ui.lock().theme.block_title_with_style(title))
// //         .borders(Borders::ALL);

// //     let context_uri = match state.ui.lock().current_page().context_uri() {
// //         None => {
// //             frame.render_widget(
// //                 Paragraph::new("Cannot determine the current page's context").block(block),
// //                 rect,
// //             );
// //             return;
// //         }
// //         Some(context_uri) => context_uri,
// //     };

// //     match state.data.read().caches.context.peek(&context_uri) {
// //         Some(context) => {
// //             frame.render_widget(block, rect);

// //             // render context description
// //             let chunks = Layout::default()
// //                 .direction(Direction::Vertical)
// //                 .margin(1)
// //                 .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
// //                 .split(rect);
// //             let page_desc = Paragraph::new(context.description())
// //                 .block(Block::default().style(state.ui.lock().theme.page_desc()));
// //             frame.render_widget(page_desc, chunks[0]);

// //             match context {
// //                 Context::Artist {
// //                     top_tracks,
// //                     albums,
// //                     related_artists,
// //                     ..
// //                 } => {
// //                     render_context_artist_widgets(
// //                         is_active,
// //                         frame,
// //                         state,
// //                         chunks[1],
// //                         (top_tracks, albums, related_artists),
// //                     );
// //                 }
// //                 Context::Playlist { tracks, .. } => {
// //                     render_track_table_widget(
// //                         frame,
// //                         chunks[1],
// //                         is_active,
// //                         state,
// //                         state.filtered_items_by_search(tracks),
// //                     );
// //                 }
// //                 Context::Album { tracks, .. } => {
// //                     render_track_table_widget(
// //                         frame,
// //                         chunks[1],
// //                         is_active,
// //                         state,
// //                         state.filtered_items_by_search(tracks),
// //                     );
// //                 }
// //             }
// //         }
// //         None => {
// //             frame.render_widget(Paragraph::new("Loading...").block(block), rect);
// //         }
// //     }
// // }

pub fn render_library_page(is_active: bool, frame: &mut Frame, state: &SharedState, rect: Rect) {
    let mut ui = state.ui.lock();
    let data = state.data.read();

    tracing::info!("reach this");

    let focus_state = match ui.current_page() {
        PageState::Library { state } => state.focus,
        _ => unreachable!("expect a library page state"),
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
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ]
            .as_ref(),
        )
        .split(rect);
    let (playlist_rect, album_rect, artist_rect) = (chunks[0], chunks[1], chunks[2]);

    // Construct the playlist window
    let playlist_list = construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.playlists)
            .into_iter()
            .map(|p| (p.name.clone(), false))
            .collect(),
        "Playlists",
        is_active && focus_state == LibraryFocusState::Playlists,
        Some((Borders::TOP | Borders::LEFT) | Borders::BOTTOM),
    );
    // Construct the saved album window
    let album_list = construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.saved_albums)
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect(),
        "Albums",
        is_active && focus_state == LibraryFocusState::SavedAlbums,
        Some((Borders::TOP | Borders::LEFT) | Borders::BOTTOM),
    );
    // Construct the followed artist window
    let artist_list = construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.followed_artists)
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect(),
        "Artists",
        is_active && focus_state == LibraryFocusState::FollowedArtists,
        None,
    );

    // Render the library page's windows.
    // Will need mutable access to the list/table states stored inside the page state for rendering.
    let page_state = match ui.current_page_mut() {
        PageState::Library { state } => state,
        _ => unreachable!("expect a library page state"),
    };
    frame.render_stateful_widget(playlist_list, playlist_rect, &mut page_state.playlist_list);
    frame.render_stateful_widget(album_list, album_rect, &mut page_state.saved_album_list);
    frame.render_stateful_widget(
        artist_list,
        artist_rect,
        &mut page_state.followed_artist_list,
    );
}

// // /// renders the recommendation window
// // pub fn render_recommendation_window(
// //     is_active: bool,
// //     frame: &mut Frame,
// //     state: &SharedState,
// //     rect: Rect,
// // ) {
// //     let seed = match state.ui.lock().current_page() {
// //         PageState::Recommendations(seed) => seed.clone(),
// //         _ => return,
// //     };

// //     let block = Block::default()
// //         .title(
// //             state
// //                 .ui
// //                 .lock()
// //                 .theme
// //                 .block_title_with_style("Recommendations"),
// //         )
// //         .borders(Borders::ALL);

// //     let data = state.data.read();

// //     let tracks = match data.caches.recommendation.peek(&seed.uri()) {
// //         Some(tracks) => tracks,
// //         None => {
// //             // recommendation tracks are still loading
// //             frame.render_widget(Paragraph::new("loading...").block(block), rect);
// //             return;
// //         }
// //     };

// //     // render the window's border and title
// //     frame.render_widget(block, rect);

// //     // render the window's description
// //     let desc = match seed {
// //         SeedItem::Track(track) => format!("{} Radio", track.name),
// //         SeedItem::Artist(artist) => format!("{} Radio", artist.name),
// //     };

// //     let chunks = Layout::default()
// //         .direction(Direction::Vertical)
// //         .margin(1)
// //         .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
// //         .split(rect);
// //     let page_desc =
// //         Paragraph::new(desc).block(Block::default().style(state.ui.lock().theme.page_desc()));
// //     frame.render_widget(page_desc, chunks[0]);

// //     render_track_table_widget(
// //         frame,
// //         chunks[1],
// //         is_active,
// //         state,
// //         state.filtered_items_by_search(tracks),
// //     );
// // }

// // /// renders the widgets for the artist context window, which includes
// // /// - A top track table
// // /// - An album list
// // /// - A related artist list
// // fn render_context_artist_widgets(
// //     is_active: bool,
// //     frame: &mut Frame,
// //     state: &SharedState,
// //     rect: Rect,
// //     data: (&[Track], &[Album], &[Artist]),
// // ) {
// //     let (tracks, albums, artists) = (
// //         state.filtered_items_by_search(data.0),
// //         state.filtered_items_by_search(data.1),
// //         state.filtered_items_by_search(data.2),
// //     );

// //     let focus_state = match state.ui.lock().window {
// //         WindowState::Artist { focus, .. } => focus,
// //         _ => {
// //             return;
// //         }
// //     };

// //     let rect = {
// //         // render the top tracks table for artist context window

// //         let chunks = Layout::default()
// //             .direction(Direction::Vertical)
// //             .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
// //             .split(rect);

// //         render_track_table_widget(
// //             frame,
// //             chunks[0],
// //             is_active && focus_state == ArtistFocusState::TopTracks,
// //             state,
// //             tracks,
// //         );

// //         chunks[1]
// //     };

// //     let chunks = Layout::default()
// //         .direction(Direction::Horizontal)
// //         .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
// //         .split(rect);

// //     // construct album list widget
// //     let album_list = {
// //         let album_items = albums
// //             .into_iter()
// //             .map(|a| (a.name.clone(), false))
// //             .collect::<Vec<_>>();

// //         construct_list_widget(
// //             state,
// //             album_items,
// //             "Albums",
// //             is_active && focus_state == ArtistFocusState::Albums,
// //             Some(Borders::TOP),
// //         )
// //     };

// //     // construct artist list widget
// //     let artist_list = {
// //         let artist_items = artists
// //             .into_iter()
// //             .map(|a| (a.name.clone(), false))
// //             .collect::<Vec<_>>();

// //         construct_list_widget(
// //             state,
// //             artist_items,
// //             "Related Artists",
// //             is_active && focus_state == ArtistFocusState::RelatedArtists,
// //             Some(Borders::TOP | Borders::LEFT),
// //         )
// //     };

// //     let mut ui = state.ui.lock();
// //     let (album_list_state, artist_list_state) = match ui.window {
// //         WindowState::Artist {
// //             ref mut album_list,
// //             ref mut related_artist_list,
// //             ..
// //         } => (album_list, related_artist_list),
// //         _ => return,
// //     };

// //     frame.render_stateful_widget(album_list, chunks[0], album_list_state);
// //     frame.render_stateful_widget(artist_list, chunks[1], artist_list_state);
// // }

// // /// renders a track table widget
// // pub fn render_track_table_widget(
// //     frame: &mut Frame,
// //     rect: Rect,
// //     is_active: bool,
// //     state: &SharedState,
// //     tracks: Vec<&Track>,
// // ) {
// //     let mut ui = state.ui.lock();

// //     // get the current playing track's URI to
// //     // highlight such track (if exists) in the track table
// //     let mut playing_track_uri = "".to_string();
// //     let mut active_desc = "";
// //     if let Some(ref playback) = state.player.read().playback {
// //         if let Some(rspotify_model::PlayableItem::Track(ref track)) = playback.item {
// //             playing_track_uri = track.id.as_ref().map(|id| id.uri()).unwrap_or_default();

// //             active_desc = if !playback.is_playing { "⏸" } else { "▶" };
// //         }
// //     }

// //     let item_max_len = state.app_config.track_table_item_max_len;
// //     let rows = tracks
// //         .into_iter()
// //         .enumerate()
// //         .map(|(id, t)| {
// //             let (id, style) = if playing_track_uri == t.id.uri() {
// //                 (active_desc.to_string(), ui.theme.current_playing())
// //             } else {
// //                 ((id + 1).to_string(), Style::default())
// //             };
// //             Row::new(vec![
// //                 Cell::from(id),
// //                 Cell::from(utils::truncate_string(t.name.clone(), item_max_len)),
// //                 Cell::from(utils::truncate_string(t.artists_info(), item_max_len)),
// //                 Cell::from(utils::truncate_string(t.album_info(), item_max_len)),
// //                 Cell::from(utils::format_duration(t.duration)),
// //             ])
// //             .style(style)
// //         })
// //         .collect::<Vec<_>>();

// //     let table = Table::new(rows)
// //         .header(
// //             Row::new(vec![
// //                 Cell::from("#"),
// //                 Cell::from("Track"),
// //                 Cell::from("Artists"),
// //                 Cell::from("Album"),
// //                 Cell::from("Duration"),
// //             ])
// //             .style(ui.theme.table_header()),
// //         )
// //         .block(Block::default())
// //         .widths(&[
// //             Constraint::Length(4),
// //             Constraint::Percentage(30),
// //             Constraint::Percentage(30),
// //             Constraint::Percentage(30),
// //             Constraint::Percentage(10),
// //         ])
// //         .highlight_style(ui.theme.selection_style(is_active));

// //     if let Some(state) = ui.window.track_table_state() {
// //         frame.render_stateful_widget(table, rect, state)
// //     }
// // }
