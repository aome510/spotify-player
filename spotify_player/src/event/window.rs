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
        Command::SearchPage => {
            ui.history.push(PageState::Searching(
                "".to_owned(),
                Box::new(SearchResults::empty()),
            ));
            ui.window = WindowState::Search(
                new_list_state(),
                new_list_state(),
                new_list_state(),
                new_list_state(),
                SearchFocusState::Input,
            );
        }
        Command::FocusNextWindow => {
            ui.window.next();
        }
        Command::FocusPreviousWindow => {
            ui.window.previous();
        }
        Command::SearchContext => {
            ui.window.select(Some(0));
            ui.popup = Some(PopupState::ContextSearch("".to_owned()));
        }
        Command::PlayContext => {
            let player = state.player.read().unwrap();
            let context = player.context();

            // randomly play a track from the current context
            if let Some(context) = context {
                if let Some(tracks) = context.tracks() {
                    let offset = match context {
                        // Spotify does not allow to manually specify `offset` for artist context
                        Context::Artist(..) => None,
                        _ => {
                            let id = rand::thread_rng().gen_range(0..tracks.len());
                            Some(model::Offset::for_uri(&tracks[id].id.uri()))
                        }
                    };

                    send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                        Some(player.context_id.clone()),
                        None,
                        offset,
                    )))?;
                }
            }
        }
        _ => {
            // handles sort/reverse commands separately
            if state.player.read().unwrap().context().is_some() {
                let sort_order = match command {
                    Command::SortTrackByTitle => Some(ContextSortOrder::TrackName),
                    Command::SortTrackByAlbum => Some(ContextSortOrder::Album),
                    Command::SortTrackByArtists => Some(ContextSortOrder::Artists),
                    Command::SortTrackByAddedDate => Some(ContextSortOrder::AddedAt),
                    Command::SortTrackByDuration => Some(ContextSortOrder::Duration),
                    _ => None,
                };
                match sort_order {
                    Some(sort_order) => {
                        state
                            .player
                            .write()
                            .unwrap()
                            .context_mut()
                            .unwrap()
                            .sort_tracks(sort_order);
                        return Ok(true);
                    }
                    None => {
                        if command == Command::ReverseTrackOrder {
                            state
                                .player
                                .write()
                                .unwrap()
                                .context_mut()
                                .unwrap()
                                .reverse_tracks();
                            return Ok(true);
                        }
                    }
                }
            }

            // the command hasn't been handled, assign the job to the focused subwindow's handler
            return handle_command_for_focused_context_subwindow(command, send, ui, state);
        }
    }
    Ok(true)
}

/// handles a key sequence for a search window
pub fn handle_key_sequence_for_search_window(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let focus_state = match ui.window {
        WindowState::Search(_, _, _, _, focus) => focus,
        _ => {
            return Ok(false);
        }
    };

    let (query, search_results) = match ui.current_page_mut() {
        PageState::Searching(ref mut query, ref mut search_results) => {
            (query, search_results.clone())
        }
        _ => unreachable!(),
    };

    // handle user's input
    if let SearchFocusState::Input = focus_state {
        if key_sequence.keys.len() == 1 {
            if let Key::None(c) = key_sequence.keys[0] {
                match c {
                    KeyCode::Char(c) => {
                        query.push(c);
                        return Ok(true);
                    }
                    KeyCode::Backspace => {
                        if !query.is_empty() {
                            query.pop().unwrap();
                        }
                        return Ok(true);
                    }
                    KeyCode::Enter => {
                        if !query.is_empty() {
                            send.send(ClientRequest::Search(query.clone()))?;
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
                let tracks = search_results.tracks.items.iter().collect::<Vec<_>>();
                handle_command_for_track_list_subwindow(command, send, ui, tracks)
            }
            SearchFocusState::Artists => {
                let artists = search_results.artists.items.iter().collect::<Vec<_>>();
                handle_command_for_artist_list_subwindow(command, send, ui, artists)
            }
            SearchFocusState::Albums => {
                let albums = search_results.albums.items.iter().collect::<Vec<_>>();
                handle_command_for_album_list_subwindow(command, send, ui, albums)
            }
            SearchFocusState::Playlists => {
                let playlists = search_results.playlists.items.iter().collect::<Vec<_>>();
                handle_command_for_playlist_list(command, send, ui, playlists)
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
    match state.player.read().unwrap().context() {
        Some(context) => match context {
            Context::Artist(_, ref tracks, ref albums, ref artists) => {
                let focus_state = match ui.window {
                    WindowState::Artist(_, _, _, state) => state,
                    _ => unreachable!(),
                };

                match focus_state {
                    ArtistFocusState::Albums => handle_command_for_album_list_subwindow(
                        command,
                        send,
                        ui,
                        ui.search_filtered_items(albums),
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list_subwindow(
                        command,
                        send,
                        ui,
                        ui.search_filtered_items(artists),
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table_subwindow(
                        command,
                        send,
                        ui,
                        None,
                        Some(tracks.iter().map(|t| t.id.uri()).collect::<Vec<_>>()),
                        ui.search_filtered_items(tracks),
                    ),
                }
            }
            Context::Album(ref album, ref tracks) => handle_command_for_track_table_subwindow(
                command,
                send,
                ui,
                Some(album.id.uri()),
                None,
                ui.search_filtered_items(tracks),
            ),
            Context::Playlist(ref playlist, ref tracks) => {
                handle_command_for_track_table_subwindow(
                    command,
                    send,
                    ui,
                    Some(playlist.id.uri()),
                    None,
                    ui.search_filtered_items(tracks),
                )
            }
            Context::Unknown(_) => Ok(false),
        },
        None => Ok(false),
    }
}

/// handles a command for the track table subwindow
///
/// In addition to the command and the application's states,
/// the function requires
/// - `tracks`: a list of tracks in the track table
/// - **either** `track_uris` (a list of track's uris) or `context_uris` (a context's uri):
///
/// If `track_uris` is specified, playing a track in the track table will
/// create a playing context consisting of tracks whose uri is in `track_uris`.
/// The above case is only used for the top track table in an **Artist** context window.
///
/// If `context_uri` is specified, playing a track in the track table will
/// create a playing context representing the context with `context_uri` uri.
/// The above case is used for the track table of a playlist or an album.
fn handle_command_for_track_table_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    context_uri: Option<String>,
    track_uris: Option<Vec<String>>,
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
            if track_uris.is_some() {
                // play a track from a list of tracks
                send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                    None,
                    track_uris,
                    Some(model::Offset::for_uri(&tracks[id].id.uri())),
                )))?;
            } else if context_uri.is_some() {
                // play a track from a context
                send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                    context_uri,
                    None,
                    Some(model::Offset::for_uri(&tracks[id].id.uri())),
                )))?;
            }
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Track(tracks[id].clone()),
                new_list_state(),
            ));
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
            send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                None,
                Some(vec![tracks[id].id.uri()]),
                None,
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Track(tracks[id].clone()),
                new_list_state(),
            ));
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
            let uri = artists[id].id.uri();
            send.send(ClientRequest::GetContext(ContextId::Artist(
                ArtistId::from_uri(&uri)?,
            )))?;
            ui.history.push(PageState::Browsing(uri));
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Artist(artists[id].clone()),
                new_list_state(),
            ));
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
            let uri = albums[id].id.uri();
            send.send(ClientRequest::GetContext(ContextId::Album(
                AlbumId::from_uri(&uri)?,
            )))?;
            ui.history.push(PageState::Browsing(uri));
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Album(albums[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn handle_command_for_playlist_list(
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
            let uri = playlists[id].id.uri();
            send.send(ClientRequest::GetContext(ContextId::Playlist(
                PlaylistId::from_uri(&uri)?,
            )))?;
            ui.history.push(PageState::Browsing(uri));
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Playlist(playlists[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
