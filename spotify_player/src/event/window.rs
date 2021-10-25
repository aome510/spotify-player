use super::*;

/// handles a key sequence for a context window
pub fn handle_key_sequence_for_context_window(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    match command {
        Command::FocusNextWindow => {
            ui.window.next();
        }
        Command::FocusPreviousWindow => {
            ui.window.previous();
        }
        Command::SearchContext => {
            ui.window.select(Some(0));
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
        }
        Command::PlayRandom => {
            let player = state.player.read().unwrap();
            let data = state.data.read().unwrap();
            let context = player.context(&data.caches);

            // randomly play a track from the current context
            if let Some(context) = context {
                let tracks = context.tracks();
                let offset = match context {
                    // Spotify does not allow to manually specify `offset` for artist context
                    Context::Artist { .. } => None,
                    _ => {
                        let id = rand::thread_rng().gen_range(0..tracks.len());
                        Some(model::Offset::for_uri(&tracks[id].id.uri()))
                    }
                };

                send.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::Context(player.context_id.clone().unwrap(), offset),
                )))?;
            }
        }
        _ => {
            // TODO: handles sort/reverse commands separately
            // if state.player.read().unwrap().context().is_some() {
            //     let sort_order = match command {
            //         Command::SortTrackByTitle => Some(ContextSortOrder::TrackName),
            //         Command::SortTrackByAlbum => Some(ContextSortOrder::Album),
            //         Command::SortTrackByArtists => Some(ContextSortOrder::Artists),
            //         Command::SortTrackByAddedDate => Some(ContextSortOrder::AddedAt),
            //         Command::SortTrackByDuration => Some(ContextSortOrder::Duration),
            //         _ => None,
            //     };
            //     match sort_order {
            //         Some(sort_order) => {
            //             state
            //                 .player
            //                 .write()
            //                 .unwrap()
            //                 .context_mut()
            //                 .unwrap()
            //                 .sort_tracks(sort_order);
            //             return Ok(true);
            //         }
            //         None => {
            //             if command == Command::ReverseTrackOrder {
            //                 state
            //                     .player
            //                     .write()
            //                     .unwrap()
            //                     .context_mut()
            //                     .unwrap()
            //                     .reverse_tracks();
            //                 return Ok(true);
            //             }
            //         }
            //     }
            // }

            // the command hasn't been handled, assign the job to the focused subwindow's handler
            return handle_command_for_focused_context_subwindow(command, send, ui, state);
        }
    }
    Ok(true)
}

/// handles a key sequence for a recommendation window
pub fn handle_key_sequence_for_recommendation_window(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let data = state.data.read().unwrap();
    let tracks = match ui.current_page() {
        PageState::Recommendations(seed) => data
            .caches
            .recommendation
            .peek(&seed.uri())
            .map(|tracks| ui.filtered_items_by_search(tracks))
            .unwrap_or_default(),
        _ => unreachable!(),
    };

    match command {
        Command::SearchContext => {
            ui.window.select(Some(0));
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
            Ok(true)
        }
        Command::PlayRandom => {
            // randomly play a song from the list of recommendation tracks
            let offset = {
                let id = rand::thread_rng().gen_range(0..tracks.len());
                Some(model::Offset::for_uri(&tracks[id].id.uri()))
            };
            send.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(tracks.iter().map(|t| t.id.clone()).collect(), offset),
            )))?;

            Ok(true)
        }
        _ => handle_command_for_track_table_subwindow(
            command,
            send,
            ui,
            None,
            Some(tracks.iter().map(|t| &t.id).collect()),
            tracks,
        ),
    }
}

/// handles a key sequence for a search window
pub fn handle_key_sequence_for_search_window(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let focus_state = match ui.window {
        WindowState::Search { focus, .. } => focus,
        _ => {
            return Ok(false);
        }
    };

    let (input, query) = match ui.current_page_mut() {
        PageState::Searching {
            input,
            current_query,
        } => (input, current_query),
        _ => unreachable!(),
    };

    // handle user's input
    if let SearchFocusState::Input = focus_state {
        if key_sequence.keys.len() == 1 {
            if let Key::None(c) = key_sequence.keys[0] {
                match c {
                    KeyCode::Char(c) => {
                        input.push(c);
                        return Ok(true);
                    }
                    KeyCode::Backspace => {
                        if !input.is_empty() {
                            input.pop().unwrap();
                        }
                        return Ok(true);
                    }
                    KeyCode::Enter => {
                        if !input.is_empty() {
                            send.send(ClientRequest::Search(input.clone()))?;
                        }
                        return Ok(true);
                    }
                    _ => {}
                }
            }
        }
    }

    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let data = state.data.read().unwrap();
    let search_results = data.caches.search.peek(query);

    match command {
        Command::FocusNextWindow => {
            ui.window.next();
            Ok(true)
        }
        Command::FocusPreviousWindow => {
            ui.window.previous();
            Ok(true)
        }
        // determine the current focused subwindow inside the search window,
        // and assign the handling job to the corresponding handler
        _ => match focus_state {
            SearchFocusState::Input => Ok(false),
            SearchFocusState::Tracks => {
                let tracks = search_results
                    .map(|s| s.tracks.iter().collect())
                    .unwrap_or_default();
                handle_command_for_track_list_subwindow(command, send, ui, tracks)
            }
            SearchFocusState::Artists => {
                let artists = search_results
                    .map(|s| s.artists.iter().collect())
                    .unwrap_or_default();
                handle_command_for_artist_list_subwindow(command, send, ui, artists)
            }
            SearchFocusState::Albums => {
                let albums = search_results
                    .map(|s| s.albums.iter().collect())
                    .unwrap_or_default();
                handle_command_for_album_list_subwindow(command, send, ui, albums)
            }
            SearchFocusState::Playlists => {
                let playlists = search_results
                    .map(|s| s.playlists.iter().collect())
                    .unwrap_or_default();
                handle_command_for_playlist_list_subwindow(command, send, ui, playlists)
            }
        },
    }
}

/// handles a command for the currently focused context subwindow
///
/// The function will need to determine the focused subwindow then
/// assign the handling job to such subwindow's command handler
pub fn handle_command_for_focused_context_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let data = state.data.read().unwrap();

    match state.player.read().unwrap().context(&data.caches) {
        Some(context) => match context {
            Context::Artist {
                top_tracks,
                albums,
                related_artists,
                ..
            } => {
                let focus_state = match ui.window {
                    WindowState::Artist { focus, .. } => focus,
                    _ => unreachable!(),
                };

                match focus_state {
                    ArtistFocusState::Albums => handle_command_for_album_list_subwindow(
                        command,
                        send,
                        ui,
                        ui.filtered_items_by_search(albums),
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list_subwindow(
                        command,
                        send,
                        ui,
                        ui.filtered_items_by_search(related_artists),
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table_subwindow(
                        command,
                        send,
                        ui,
                        None,
                        Some(top_tracks.iter().map(|t| &t.id).collect()),
                        ui.filtered_items_by_search(top_tracks),
                    ),
                }
            }
            Context::Album { album, tracks } => handle_command_for_track_table_subwindow(
                command,
                send,
                ui,
                Some(ContextId::Album(album.id.clone())),
                None,
                ui.filtered_items_by_search(tracks),
            ),
            Context::Playlist { playlist, tracks } => handle_command_for_track_table_subwindow(
                command,
                send,
                ui,
                Some(ContextId::Playlist(playlist.id.clone())),
                None,
                ui.filtered_items_by_search(tracks),
            ),
        },
        None => Ok(false),
    }
}

/// handles a command for the track table subwindow
///
/// In addition to the command and the application's states,
/// the function requires
/// - `tracks`: a list of tracks in the track table (can already be filtered by a search query)
/// - **either** `track_ids` or `context_id`
///
/// If `track_ids` is specified, playing a track in the track table will
/// start a `URIs` playback consisting of tracks whose id is in `track_ids`.
/// The above case is only used for the top-track table in an **Artist** context window.
///
/// If `context_id` is specified, playing a track in the track table will
/// start a `Context` playback representing a Spotify context.
/// The above case is used for the track table of a playlist or an album.
fn handle_command_for_track_table_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    context_id: Option<ContextId>,
    track_ids: Option<Vec<&TrackId>>,
    tracks: Vec<&Track>,
) -> Result<bool> {
    let id = ui.window.selected().unwrap();

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < tracks.len() {
                ui.window.select(Some(id + 1));
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.window.select(Some(id - 1));
            }
        }
        Command::ChooseSelected => {
            let offset = Some(model::Offset::for_uri(&tracks[id].id.uri()));
            if track_ids.is_some() {
                // play a track from a list of tracks
                send.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::URIs(track_ids.unwrap().into_iter().cloned().collect(), offset),
                )))?;
            } else if context_id.is_some() {
                // play a track from a context
                send.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::Context(context_id.unwrap(), offset),
                )))?;
            }
        }
        Command::ShowActionsOnSelectedItem => {
            let item = Item::Track(tracks[id].clone());
            let actions = item.actions();
            ui.popup = Some(PopupState::ActionList(item, actions, new_list_state()));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn handle_command_for_track_list_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    tracks: Vec<&Track>,
) -> Result<bool> {
    let id = ui.window.selected().unwrap();

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < tracks.len() {
                ui.window.select(Some(id + 1));
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.window.select(Some(id - 1));
            }
        }
        Command::ChooseSelected => {
            // for the track list, `ChooseSelected` on a track
            // will start a `URIs` playback containing only that track.
            // It's different for the track table, in which
            // `ChooseSelected` on a track will start a `URIs` playback
            // containing all the tracks in the table.
            send.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(vec![tracks[id].id.clone()], None),
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let item = Item::Track(tracks[id].clone());
            let actions = item.actions();
            ui.popup = Some(PopupState::ActionList(item, actions, new_list_state()));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn handle_command_for_artist_list_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    artists: Vec<&Artist>,
) -> Result<bool> {
    let id = ui.window.selected().unwrap();

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < artists.len() {
                ui.window.select(Some(id + 1));
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.window.select(Some(id - 1));
            }
        }
        Command::ChooseSelected => {
            let context_id = ContextId::Artist(artists[id].id.clone());
            send.send(ClientRequest::GetContext(context_id.clone()))?;
            ui.create_new_page(PageState::Browsing(context_id));
        }
        Command::ShowActionsOnSelectedItem => {
            let item = Item::Artist(artists[id].clone());
            let actions = item.actions();
            ui.popup = Some(PopupState::ActionList(item, actions, new_list_state()));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn handle_command_for_album_list_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    albums: Vec<&Album>,
) -> Result<bool> {
    let id = ui.window.selected().unwrap();

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < albums.len() {
                ui.window.select(Some(id + 1));
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.window.select(Some(id - 1));
            }
        }
        Command::ChooseSelected => {
            let context_id = ContextId::Album(albums[id].id.clone());
            send.send(ClientRequest::GetContext(context_id.clone()))?;
            ui.create_new_page(PageState::Browsing(context_id));
        }
        Command::ShowActionsOnSelectedItem => {
            let item = Item::Album(albums[id].clone());
            let actions = item.actions();
            ui.popup = Some(PopupState::ActionList(item, actions, new_list_state()));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn handle_command_for_playlist_list_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    playlists: Vec<&Playlist>,
) -> Result<bool> {
    let id = ui.window.selected().unwrap();

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < playlists.len() {
                ui.window.select(Some(id + 1));
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.window.select(Some(id - 1));
            }
        }
        Command::ChooseSelected => {
            let context_id = ContextId::Playlist(playlists[id].id.clone());
            send.send(ClientRequest::GetContext(context_id.clone()))?;
            ui.create_new_page(PageState::Browsing(context_id));
        }
        Command::ShowActionsOnSelectedItem => {
            let item = Item::Playlist(playlists[id].clone());
            let actions = item.actions();
            ui.popup = Some(PopupState::ActionList(item, actions, new_list_state()));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
