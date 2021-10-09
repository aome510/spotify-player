use super::*;

pub fn handle_key_sequence_for_context_window(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
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

            // needs to set `context_uri` to an empty string
            // to trigger updating the context window when going
            // backward from a page (call `PreviousPage` command)
            state.player.write().unwrap().context_uri = "".to_owned();
            Ok(true)
        }
        Command::FocusNextWindow => {
            ui.window.next();
            Ok(true)
        }
        Command::FocusPreviousWindow => {
            ui.window.previous();
            Ok(true)
        }
        Command::SearchContext => {
            ui.window.select(Some(0));
            ui.popup = PopupState::ContextSearch("".to_owned());
            Ok(true)
        }
        Command::PlayContext => {
            let player = state.player.read().unwrap();
            let context = player.get_context();

            // randomly play a track from the current context
            if let Some(context) = context {
                if let Some(tracks) = context.get_tracks() {
                    let offset = match context {
                        // Spotify does not allow to manually specify `offset` for artist context
                        Context::Artist(..) => None,
                        _ => {
                            let id = rand::thread_rng().gen_range(0..tracks.len());
                            offset::for_uri(tracks[id].uri.clone())
                        }
                    };
                    send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                        Some(player.context_uri.clone()),
                        None,
                        offset,
                    )))?;
                }
            }
            Ok(true)
        }
        _ => {
            let handled = {
                if state.player.read().unwrap().get_context().is_none() {
                    false
                } else {
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
                                .get_context_mut()
                                .unwrap()
                                .sort_tracks(sort_order);
                            true
                        }
                        None => {
                            if command == Command::ReverseTrackOrder {
                                state
                                    .player
                                    .write()
                                    .unwrap()
                                    .get_context_mut()
                                    .unwrap()
                                    .reverse_tracks();
                                true
                            } else {
                                false
                            }
                        }
                    }
                }
            };
            if handled {
                Ok(true)
            } else {
                handle_command_for_focused_context_subwindow(command, send, ui, state)
            }
        }
    }
}

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

    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    if let Some(command) = command {
        match command {
            Command::FocusNextWindow => {
                ui.window.next();
                return Ok(true);
            }
            Command::FocusPreviousWindow => {
                ui.window.previous();
                return Ok(true);
            }
            _ => match focus_state {
                SearchFocusState::Input => {}
                SearchFocusState::Tracks => {
                    let tracks = search_results.tracks.items.iter().collect::<Vec<_>>();
                    return handle_command_for_track_list(command, send, ui, tracks);
                }
                SearchFocusState::Artists => {
                    let artists = search_results.artists.items.iter().collect::<Vec<_>>();
                    return handle_command_for_artist_list(command, send, ui, artists);
                }
                SearchFocusState::Albums => {
                    let albums = search_results.albums.items.iter().collect::<Vec<_>>();
                    return handle_command_for_album_list(command, send, ui, albums);
                }
                SearchFocusState::Playlists => {
                    let playlists = search_results.playlists.items.iter().collect::<Vec<_>>();
                    return handle_command_for_playlist_list(command, send, ui, playlists);
                }
            },
        }
    }

    Ok(false)
}

pub fn handle_command_for_focused_context_subwindow(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    match state.player.read().unwrap().get_context() {
        Some(context) => match context {
            Context::Artist(_, ref tracks, ref albums, ref artists) => {
                let focus_state = match ui.window {
                    WindowState::Artist(_, _, _, state) => state,
                    _ => unreachable!(),
                };
                match focus_state {
                    ArtistFocusState::Albums => handle_command_for_album_list(
                        command,
                        send,
                        ui,
                        ui.get_search_filtered_items(albums),
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list(
                        command,
                        send,
                        ui,
                        ui.get_search_filtered_items(artists),
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table(
                        command,
                        send,
                        ui,
                        None,
                        Some(tracks.iter().map(|t| t.uri.clone()).collect::<Vec<_>>()),
                        ui.get_search_filtered_items(tracks),
                    ),
                }
            }
            Context::Album(ref album, ref tracks) => handle_command_for_track_table(
                command,
                send,
                ui,
                Some(album.uri.clone()),
                None,
                ui.get_search_filtered_items(tracks),
            ),
            Context::Playlist(ref playlist, ref tracks) => handle_command_for_track_table(
                command,
                send,
                ui,
                Some(playlist.uri.clone()),
                None,
                ui.get_search_filtered_items(tracks),
            ),
            Context::Unknown(_) => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_command_for_track_list(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    tracks: Vec<&Track>,
) -> Result<bool> {
    match command {
        Command::SelectNextOrScrollDown => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < tracks.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPreviousOrScrollUp => {
            if let Some(id) = ui.window.selected() {
                if id > 0 {
                    ui.window.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.window.selected() {
                send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                    None,
                    Some(vec![tracks[id].uri.clone()]),
                    None,
                )))?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_artist_list(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    artists: Vec<&Artist>,
) -> Result<bool> {
    match command {
        Command::SelectNextOrScrollDown => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < artists.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPreviousOrScrollUp => {
            if let Some(id) = ui.window.selected() {
                if id > 0 {
                    ui.window.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.window.selected() {
                let uri = artists[id].uri.clone().unwrap();
                send.send(ClientRequest::GetContext(ContextURI::Artist(uri.clone())))?;
                ui.history.push(PageState::Browsing(uri));
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_album_list(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    albums: Vec<&Album>,
) -> Result<bool> {
    match command {
        Command::SelectNextOrScrollDown => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < albums.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPreviousOrScrollUp => {
            if let Some(id) = ui.window.selected() {
                if id > 0 {
                    ui.window.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.window.selected() {
                let uri = albums[id].uri.clone().unwrap();
                send.send(ClientRequest::GetContext(ContextURI::Album(uri.clone())))?;
                ui.history.push(PageState::Browsing(uri));
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_playlist_list(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    playlists: Vec<&playlist::SimplifiedPlaylist>,
) -> Result<bool> {
    match command {
        Command::SelectNextOrScrollDown => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < playlists.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPreviousOrScrollUp => {
            if let Some(id) = ui.window.selected() {
                if id > 0 {
                    ui.window.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.window.selected() {
                let uri = playlists[id].uri.clone();
                send.send(ClientRequest::GetContext(ContextURI::Playlist(uri.clone())))?;
                ui.history.push(PageState::Browsing(uri));
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_track_table(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    context_uri: Option<String>,
    track_uris: Option<Vec<String>>,
    tracks: Vec<&Track>,
) -> Result<bool> {
    match command {
        Command::SelectNextOrScrollDown => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < tracks.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPreviousOrScrollUp => {
            if let Some(id) = ui.window.selected() {
                if id > 0 {
                    ui.window.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.window.selected() {
                if track_uris.is_some() {
                    // play a track from a list of tracks, use ID offset for finding the track
                    send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                        None,
                        track_uris,
                        offset::for_position(id as u32),
                    )))?;
                } else if context_uri.is_some() {
                    // play a track from a context, use URI offset for finding the track
                    send.send(ClientRequest::Player(PlayerRequest::PlayTrack(
                        context_uri,
                        None,
                        offset::for_uri(tracks[id].uri.clone()),
                    )))?;
                }
            }
            Ok(true)
        }
        Command::BrowseSelectedTrackAlbum => {
            if let Some(id) = ui.window.selected() {
                if let Some(ref uri) = tracks[id].album.uri {
                    send.send(ClientRequest::GetContext(ContextURI::Album(uri.clone())))?;
                    ui.history.push(PageState::Browsing(uri.clone()));
                }
            }
            Ok(true)
        }
        Command::BrowseSelectedTrackArtists => {
            if let Some(id) = ui.window.selected() {
                let artists = tracks[id]
                    .artists
                    .iter()
                    .map(|a| Artist {
                        name: a.name.clone(),
                        uri: a.uri.clone(),
                        id: a.id.clone(),
                    })
                    .filter(|a| a.uri.is_some())
                    .collect::<Vec<_>>();
                ui.popup = PopupState::ArtistList(artists, new_list_state());
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
