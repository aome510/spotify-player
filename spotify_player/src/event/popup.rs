use super::*;

/// handles a key sequence for a popup
pub fn handle_key_sequence_for_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match ui.popup.as_ref().unwrap() {
        PopupState::ContextSearch(_) => {
            handle_key_sequence_for_search_popup(key_sequence, send, state, ui)
        }
        PopupState::ArtistList(..) => handle_key_sequence_for_list_popup(
            key_sequence,
            state,
            ui,
            match ui.popup {
                Some(PopupState::ArtistList(ref artists, _)) => artists.len(),
                _ => unreachable!(),
            },
            |_, _| {},
            |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                let artists = match ui.popup {
                    Some(PopupState::ArtistList(ref artists, _)) => artists,
                    _ => unreachable!(),
                };
                let uri = artists[id].uri.clone().unwrap();

                send.send(ClientRequest::GetContext(ContextURI::Artist(uri.clone())))?;

                ui.history.push(PageState::Browsing(uri));
                ui.popup = None;
                Ok(())
            },
            |ui: &mut UIStateGuard| {
                ui.popup = None;
            },
        ),
        PopupState::UserPlaylistList(_) => {
            let player = state.player.read().unwrap();
            let playlist_uris = player
                .user_playlists
                .iter()
                .map(|p| p.uri.clone())
                .collect::<Vec<_>>();

            handle_key_sequence_for_context_list_popup(
                key_sequence,
                send,
                state,
                ui,
                playlist_uris,
                ContextURI::Playlist("".to_owned()),
            )
        }
        PopupState::UserFollowedArtistList(_) => {
            let player = state.player.read().unwrap();
            let artist_uris = player
                .user_followed_artists
                .iter()
                .map(|a| a.uri.clone().unwrap())
                .collect::<Vec<_>>();

            handle_key_sequence_for_context_list_popup(
                key_sequence,
                send,
                state,
                ui,
                artist_uris,
                ContextURI::Artist("".to_owned()),
            )
        }
        PopupState::UserSavedAlbumList(_) => {
            let player = state.player.read().unwrap();
            let album_uris = player
                .user_saved_albums
                .iter()
                .map(|a| a.uri.clone().unwrap())
                .collect::<Vec<_>>();

            handle_key_sequence_for_context_list_popup(
                key_sequence,
                send,
                state,
                ui,
                album_uris,
                ContextURI::Album("".to_owned()),
            )
        }
        PopupState::ThemeList(_, _) => handle_key_sequence_for_list_popup(
            key_sequence,
            state,
            ui,
            match ui.popup {
                Some(PopupState::ThemeList(ref themes, _)) => themes.len(),
                _ => unreachable!(),
            },
            |ui: &mut UIStateGuard, id: usize| {
                ui.theme = match ui.popup {
                    Some(PopupState::ThemeList(ref themes, _)) => themes[id].clone(),
                    _ => unreachable!(),
                };
            },
            |ui: &mut UIStateGuard, _| -> Result<()> {
                ui.popup = None;
                Ok(())
            },
            |ui: &mut UIStateGuard| {
                ui.theme = match ui.popup {
                    Some(PopupState::ThemeList(ref themes, _)) => themes[0].clone(),
                    _ => unreachable!(),
                };
                ui.popup = None;
            },
        ),
        PopupState::DeviceList(_) => {
            let player = state.player.read().unwrap();

            handle_key_sequence_for_list_popup(
                key_sequence,
                state,
                ui,
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
        PopupState::CommandHelp(_) => {
            handle_key_sequence_for_command_help_popup(key_sequence, state, ui)
        }
        PopupState::ActionList(..) => {
            todo!()
        }
    }
}

/// handles a key sequence for a context search popup
fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let query = match ui.popup {
        Some(PopupState::ContextSearch(ref mut query)) => query,
        _ => unreachable!(),
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
            }
            Command::FocusNextWindow => {
                ui.window.next();
            }
            Command::FocusPreviousWindow => {
                ui.window.previous();
            }
            _ => {
                return window::handle_command_for_focused_context_subwindow(
                    command, send, ui, state,
                )
            }
        },
        None => return Ok(false),
    }
    Ok(true)
}

/// handles a key sequence for a context list popup in which
/// each item represents a context
///
/// In addition to application's states and the key sequence,
/// the function requires to specify:
/// - `uris`: a list of context URIs
/// - `uri_type`: an enum represents the type of a context in the list (`playlist`, `artist`, etc)
fn handle_key_sequence_for_context_list_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
    uris: Vec<String>,
    uri_type: ContextURI,
) -> Result<bool> {
    handle_key_sequence_for_list_popup(
        key_sequence,
        state,
        ui,
        uris.len(),
        |_, _| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            let uri = uris[id].clone();
            let context_uri = match uri_type {
                ContextURI::Playlist(_) => ContextURI::Playlist(uri),
                ContextURI::Artist(_) => ContextURI::Artist(uri),
                ContextURI::Album(_) => ContextURI::Album(uri),
                ContextURI::Unknown(_) => ContextURI::Unknown(uri),
            };

            send.send(ClientRequest::GetContext(context_uri))?;

            ui.history.push(PageState::Browsing(uris[id].clone()));
            ui.popup = None;
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
    ui: &mut UIStateGuard,
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

    let popup = ui.popup.as_mut().unwrap();
    let current_id = popup.list_selected().unwrap();

    match command {
        Command::SelectPreviousOrScrollUp => {
            if current_id > 0 {
                popup.list_select(Some(current_id - 1));
                on_select_func(ui, current_id - 1);
            }
        }
        Command::SelectNextOrScrollDown => {
            if current_id + 1 < n_items {
                popup.list_select(Some(current_id + 1));
                on_select_func(ui, current_id + 1);
            }
        }
        Command::ChooseSelected => {
            on_choose_func(ui, current_id)?;
        }
        Command::ClosePopup => {
            on_close_func(ui);
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
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let offset = match ui.popup {
        Some(PopupState::CommandHelp(ref mut offset)) => offset,
        _ => unreachable!(),
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
