use crate::command::Action;

use super::*;

/// handles a key sequence for a popup
pub fn handle_key_sequence_for_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let ui = state.ui.lock();

    match ui.popup.as_ref().unwrap() {
        PopupState::Search { .. } => {
            drop(ui);
            handle_key_sequence_for_search_popup(key_sequence, send, state)
        }
        PopupState::ArtistList(artists, _) => {
            let n_items = artists.len();

            drop(ui);
            handle_key_sequence_for_list_popup(
                key_sequence,
                state,
                n_items,
                |_, _| {},
                |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                    let artists = match ui.popup {
                        Some(PopupState::ArtistList(ref artists, _)) => artists,
                        _ => return Ok(()),
                    };

                    let context_id = ContextId::Artist(artists[id].id.clone());
                    ui.create_new_page(PageState::Context(
                        None,
                        ContextPageType::Browsing(context_id),
                    ));

                    Ok(())
                },
                |ui: &mut UIStateGuard| {
                    ui.popup = None;
                },
            )
        }
        PopupState::UserPlaylistList(action, _) => {
            match action {
                PlaylistPopupAction::Browse => {
                    let playlist_uris = state
                        .data
                        .read()
                        .user_data
                        .playlists
                        .iter()
                        .map(|p| p.id.uri())
                        .collect::<Vec<_>>();

                    drop(ui);
                    handle_key_sequence_for_context_browsing_list_popup(
                        key_sequence,
                        state,
                        playlist_uris,
                        rspotify_model::Type::Playlist,
                    )
                }
                PlaylistPopupAction::AddTrack(track_id) => {
                    let track_id = track_id.clone();
                    let playlist_ids = state
                        .data
                        .read()
                        .user_data
                        .playlists_created_by_user()
                        .into_iter()
                        .map(|p| p.id.clone())
                        .collect::<Vec<_>>();

                    drop(ui);
                    handle_key_sequence_for_list_popup(
                        key_sequence,
                        state,
                        playlist_ids.len(),
                        |_, _| {},
                        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                            // when adding a new track to a playlist, we need to remove
                            // the cache for that playlist
                            state
                                .data
                                .write()
                                .caches
                                .context
                                .pop(&playlist_ids[id].uri());

                            send.send(ClientRequest::AddTrackToPlaylist(
                                playlist_ids[id].clone(),
                                track_id.clone(),
                            ))?;
                            ui.popup = None;
                            Ok(())
                        },
                        |ui: &mut UIStateGuard| {
                            ui.popup = None;
                        },
                    )
                }
            }
        }
        PopupState::UserFollowedArtistList(_) => {
            let artist_uris = state
                .data
                .read()
                .user_data
                .followed_artists
                .iter()
                .map(|a| a.id.uri())
                .collect::<Vec<_>>();

            drop(ui);
            handle_key_sequence_for_context_browsing_list_popup(
                key_sequence,
                state,
                artist_uris,
                rspotify_model::Type::Artist,
            )
        }
        PopupState::UserSavedAlbumList(_) => {
            let album_uris = state
                .data
                .read()
                .user_data
                .saved_albums
                .iter()
                .map(|a| a.id.uri())
                .collect::<Vec<_>>();

            drop(ui);
            handle_key_sequence_for_context_browsing_list_popup(
                key_sequence,
                state,
                album_uris,
                rspotify_model::Type::Album,
            )
        }
        PopupState::ThemeList(themes, _) => {
            let n_items = themes.len();

            drop(ui);
            handle_key_sequence_for_list_popup(
                key_sequence,
                state,
                n_items,
                |ui: &mut UIStateGuard, id: usize| {
                    ui.theme = match ui.popup {
                        Some(PopupState::ThemeList(ref themes, _)) => themes[id].clone(),
                        _ => return,
                    };
                },
                |ui: &mut UIStateGuard, _| -> Result<()> {
                    ui.popup = None;
                    Ok(())
                },
                |ui: &mut UIStateGuard| {
                    ui.theme = match ui.popup {
                        Some(PopupState::ThemeList(ref themes, _)) => themes[0].clone(),
                        _ => return,
                    };
                    ui.popup = None;
                },
            )
        }
        PopupState::DeviceList(_) => {
            let player = state.player.read();

            drop(ui);
            handle_key_sequence_for_list_popup(
                key_sequence,
                state,
                player.devices.len(),
                |_, _| {},
                |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                    send.send(ClientRequest::Player(PlayerRequest::TransferPlayback(
                        player.devices[id].id.clone(),
                        true,
                    )))?;
                    ui.popup = None;
                    Ok(())
                },
                |ui: &mut UIStateGuard| {
                    ui.popup = None;
                },
            )
        }
        PopupState::CommandHelp { .. } => {
            drop(ui);
            handle_key_sequence_for_command_help_popup(key_sequence, state)
        }
        PopupState::ActionList(item, ..) => {
            let actions = item.actions();
            drop(ui);
            handle_key_sequence_for_action_list_popup(actions, key_sequence, send, state)
        }
    }
}

/// handles a key sequence for a context search popup
fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

    // handle user's input that updates the search query
    let query = match ui.popup {
        Some(PopupState::Search { ref mut query }) => query,
        _ => return Ok(false),
    };
    if key_sequence.keys.len() == 1 {
        if let Key::None(c) = key_sequence.keys[0] {
            match c {
                KeyCode::Char(c) => {
                    query.push(c);
                    ui.window.select(Some(0));
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    if !query.is_empty() {
                        query.pop().unwrap();
                        ui.window.select(Some(0));
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
    }

    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::ClosePopup => {
                ui.window.select(Some(0));
                ui.popup = None;
                Ok(true)
            }
            _ => match ui.current_page() {
                PageState::Library => {
                    drop(ui);
                    window::handle_key_sequence_for_library_window(key_sequence, state)
                }
                PageState::Recommendations(..) => {
                    drop(ui);
                    window::handle_key_sequence_for_recommendation_window(key_sequence, send, state)
                }
                PageState::Context(..) => {
                    drop(ui);
                    window::handle_key_sequence_for_context_window(key_sequence, send, state)
                }
                PageState::Searching { .. } => Ok(false),
            },
        },
        None => Ok(false),
    }
}

/// handles a key sequence for a context list popup in which
/// each item represents a context
///
/// In addition to application's states and the key sequence,
/// the function requires to specify:
/// - `uris`: a list of context URIs
/// - `uri_type`: an enum represents the type of a context in the list (`playlist`, `artist`, etc)
fn handle_key_sequence_for_context_browsing_list_popup(
    key_sequence: &KeySequence,
    state: &SharedState,
    uris: Vec<String>,
    context_type: rspotify_model::Type,
) -> Result<bool> {
    handle_key_sequence_for_list_popup(
        key_sequence,
        state,
        uris.len(),
        |_, _| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            let uri = uris[id].clone();
            let context_id = match context_type {
                rspotify_model::Type::Playlist => ContextId::Playlist(PlaylistId::from_uri(&uri)?),
                rspotify_model::Type::Artist => ContextId::Artist(ArtistId::from_uri(&uri)?),
                rspotify_model::Type::Album => ContextId::Album(AlbumId::from_uri(&uri)?),
                _ => {
                    return Ok(());
                }
            };

            ui.create_new_page(PageState::Context(
                None,
                ContextPageType::Browsing(context_id),
            ));

            Ok(())
        },
        |ui: &mut UIStateGuard| {
            ui.popup = None;
        },
    )
}

/// handles a key sequence for a generic list popup.
///
/// In addition the the application states and the key sequence,
/// the function requires to specify
/// - `n_items`: the number of items in the list
/// - `on_select_func`: the callback when selecting an item
/// - `on_choose_func`: the callback when choosing an item
/// - `on_close_func`: the callback when closing the popup
fn handle_key_sequence_for_list_popup(
    key_sequence: &KeySequence,
    state: &SharedState,
    n_items: usize,
    on_select_func: impl Fn(&mut UIStateGuard, usize),
    on_choose_func: impl Fn(&mut UIStateGuard, usize) -> Result<()>,
    on_close_func: impl Fn(&mut UIStateGuard),
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let mut ui = state.ui.lock();

    let popup = ui.popup.as_mut().unwrap();
    let current_id = popup.list_selected().unwrap_or_default();

    match command {
        Command::SelectPreviousOrScrollUp => {
            if current_id > 0 {
                popup.list_select(Some(current_id - 1));
                on_select_func(&mut ui, current_id - 1);
            }
        }
        Command::SelectNextOrScrollDown => {
            if current_id + 1 < n_items {
                popup.list_select(Some(current_id + 1));
                on_select_func(&mut ui, current_id + 1);
            }
        }
        Command::ChooseSelected => {
            if current_id < n_items {
                on_choose_func(&mut ui, current_id)?;
            }
        }
        Command::ClosePopup => {
            on_close_func(&mut ui);
        }
        _ => {
            return Ok(false);
        }
    };
    Ok(true)
}

/// handles a key sequence for a command shortcut help popup
fn handle_key_sequence_for_command_help_popup(
    key_sequence: &KeySequence,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let mut ui = state.ui.lock();

    let offset = match ui.popup {
        Some(PopupState::CommandHelp { ref mut offset }) => offset,
        _ => return Ok(false),
    };
    match command {
        Command::ClosePopup => {
            ui.popup = None;
        }
        Command::SelectNextOrScrollDown => {
            *offset += 1;
        }
        Command::SelectPreviousOrScrollUp => {
            if *offset > 0 {
                *offset -= 1;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// handles a key sequence for an action list popup
fn handle_key_sequence_for_action_list_popup(
    actions: Vec<Action>,
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    handle_key_sequence_for_list_popup(
        key_sequence,
        state,
        actions.len(),
        |_, _| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            let item = match ui.popup {
                Some(PopupState::ActionList(ref item, ..)) => item,
                _ => return Ok(()),
            };

            match item {
                Item::Track(track) => match actions[id] {
                    Action::BrowseAlbum => {
                        if let Some(ref album) = track.album {
                            let uri = album.id.uri();
                            let context_id = ContextId::Album(AlbumId::from_uri(&uri)?);
                            ui.create_new_page(PageState::Context(
                                None,
                                ContextPageType::Browsing(context_id),
                            ));
                        }
                    }
                    Action::BrowseArtist => {
                        ui.popup = Some(PopupState::ArtistList(
                            track.artists.clone(),
                            new_list_state(),
                        ));
                    }
                    Action::AddTrackToPlaylist => {
                        send.send(ClientRequest::GetUserPlaylists)?;
                        ui.popup = Some(PopupState::UserPlaylistList(
                            PlaylistPopupAction::AddTrack(track.id.clone()),
                            new_list_state(),
                        ));
                    }
                    Action::SaveToLibrary => {
                        send.send(ClientRequest::SaveToLibrary(item.clone()))?;
                        ui.popup = None;
                    }
                    Action::BrowseRecommendations => {
                        let seed = SeedItem::Track(track.clone());
                        send.send(ClientRequest::GetRecommendations(seed.clone()))?;
                        ui.create_new_page(PageState::Recommendations(seed));
                    }
                },
                Item::Album(album) => match actions[id] {
                    Action::BrowseArtist => {
                        ui.popup = Some(PopupState::ArtistList(
                            album.artists.clone(),
                            new_list_state(),
                        ));
                    }
                    Action::SaveToLibrary => {
                        send.send(ClientRequest::SaveToLibrary(item.clone()))?;
                        ui.popup = None;
                    }
                    _ => {}
                },
                Item::Artist(artist) => match actions[id] {
                    Action::SaveToLibrary => {
                        send.send(ClientRequest::SaveToLibrary(item.clone()))?;
                        ui.popup = None;
                    }
                    Action::BrowseRecommendations => {
                        let seed = SeedItem::Artist(artist.clone());
                        send.send(ClientRequest::GetRecommendations(seed.clone()))?;
                        ui.create_new_page(PageState::Recommendations(seed));
                    }
                    _ => {}
                },
                Item::Playlist(_) => {
                    if let Action::SaveToLibrary = actions[id] {
                        send.send(ClientRequest::SaveToLibrary(item.clone()))?;
                        ui.popup = None;
                    }
                }
            }
            Ok(())
        },
        |ui: &mut UIStateGuard| {
            ui.popup = None;
        },
    )
}
