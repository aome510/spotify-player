use super::*;
use crate::command::construct_artist_actions;
use anyhow::Context;

pub fn handle_key_sequence_for_popup(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let popup = ui.popup.as_ref().context("empty popup")?;

    // handle popups that need reading the raw key sequence instead of the matched command
    match popup {
        PopupState::Search { .. } => {
            return handle_key_sequence_for_search_popup(key_sequence, client_pub, state, ui);
        }
        PopupState::PlaylistCreate { .. } => {
            return handle_key_sequence_for_create_playlist_popup(key_sequence, client_pub, ui);
        }
        PopupState::ActionList(item, ..) => {
            return handle_key_sequence_for_action_list_popup(
                item.n_actions(),
                key_sequence,
                client_pub,
                state,
                ui,
            );
        }
        _ => {}
    }

    let command = match config::get_config()
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    match popup {
        PopupState::Search { .. } => anyhow::bail!("search popup should be handled before"),
        PopupState::PlaylistCreate { .. } => {
            anyhow::bail!("create playlist popup should be handled before")
        }
        PopupState::ActionList(..) => {
            anyhow::bail!("action list popup should be handled before")
        }
        PopupState::ArtistList(_, artists, _) => {
            let n_items = artists.len();

            handle_command_for_list_popup(
                command,
                ui,
                n_items,
                |_, _| {},
                |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                    let (action, artists) = match ui.popup {
                        Some(PopupState::ArtistList(action, ref artists, _)) => (action, artists),
                        _ => return Ok(()),
                    };

                    match action {
                        ArtistPopupAction::Browse => {
                            let context_id = ContextId::Artist(artists[id].id.clone());
                            ui.new_page(PageState::Context {
                                id: None,
                                context_page_type: ContextPageType::Browsing(context_id),
                                state: None,
                            });
                        }
                        ArtistPopupAction::ShowActions => {
                            let actions = {
                                let data = state.data.read();
                                construct_artist_actions(&artists[id], &data)
                            };
                            ui.popup = Some(PopupState::ActionList(
                                ActionListItem::Artist(artists[id].clone(), actions),
                                new_list_state(),
                            ));
                        }
                    }

                    Ok(())
                },
                |ui: &mut UIStateGuard| {
                    ui.popup = None;
                },
            )
        }
        PopupState::UserPlaylistList(action, _) => match action {
            PlaylistPopupAction::Browse => {
                let playlist_uris = state
                    .data
                    .read()
                    .user_data
                    .playlists
                    .iter()
                    .map(|p| p.id.uri())
                    .collect::<Vec<_>>();

                handle_command_for_context_browsing_list_popup(
                    command,
                    ui,
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
                    .modifiable_playlists()
                    .into_iter()
                    .map(|p| p.id.clone())
                    .collect::<Vec<_>>();

                handle_command_for_list_popup(
                    command,
                    ui,
                    playlist_ids.len(),
                    |_, _| {},
                    |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                        client_pub.send(ClientRequest::AddTrackToPlaylist(
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
        },
        PopupState::UserFollowedArtistList(_) => {
            let artist_uris = state
                .data
                .read()
                .user_data
                .followed_artists
                .iter()
                .map(|a| a.id.uri())
                .collect::<Vec<_>>();

            handle_command_for_context_browsing_list_popup(
                command,
                ui,
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

            handle_command_for_context_browsing_list_popup(
                command,
                ui,
                album_uris,
                rspotify_model::Type::Album,
            )
        }
        PopupState::ThemeList(themes, _) => {
            let n_items = themes.len();

            handle_command_for_list_popup(
                command,
                ui,
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

            handle_command_for_list_popup(
                command,
                ui,
                player.devices.len(),
                |_, _| {},
                |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                    let is_playing = player
                        .playback
                        .as_ref()
                        .map(|p| p.is_playing)
                        .unwrap_or(false);
                    client_pub.send(ClientRequest::Player(PlayerRequest::TransferPlayback(
                        player.devices[id].id.clone(),
                        is_playing,
                    )))?;
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

fn handle_key_sequence_for_create_playlist_popup(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let (name, desc, current_field) = match ui.popup {
        Some(PopupState::PlaylistCreate {
            ref mut name,
            ref mut desc,
            ref mut current_field,
        }) => (name, desc, current_field),
        _ => return Ok(false),
    };
    if key_sequence.keys.len() == 1 {
        match &key_sequence.keys[0] {
            Key::None(crossterm::event::KeyCode::Enter) => {
                client_pub.send(ClientRequest::CreatePlaylist {
                    playlist_name: name.get_text(),
                    public: false,
                    collab: false,
                    desc: desc.get_text(),
                })?;
                ui.popup = None;
                return Ok(true);
            }
            Key::None(crossterm::event::KeyCode::Tab)
            | Key::None(crossterm::event::KeyCode::BackTab) => {
                *current_field = match &current_field {
                    PlaylistCreateCurrentField::Name => PlaylistCreateCurrentField::Desc,
                    PlaylistCreateCurrentField::Desc => PlaylistCreateCurrentField::Name,
                };
                return Ok(true);
            }
            k => {
                let line_input = match current_field {
                    PlaylistCreateCurrentField::Name => name,
                    PlaylistCreateCurrentField::Desc => desc,
                };
                if line_input.input(k).is_some() {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    // handle user's input that updates the search query
    let query = match ui.popup {
        Some(PopupState::Search { ref mut query }) => query,
        _ => return Ok(false),
    };
    if key_sequence.keys.len() == 1 {
        if let Key::None(c) = key_sequence.keys[0] {
            match c {
                crossterm::event::KeyCode::Char(c) => {
                    query.push(c);
                    ui.current_page_mut().select(0);
                    return Ok(true);
                }
                crossterm::event::KeyCode::Backspace => {
                    if !query.is_empty() {
                        query.pop().unwrap();
                        ui.current_page_mut().select(0);
                    } else {
                        // close search popup when user presses backspace on empty search
                        ui.popup = None;
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
    }

    // key sequence not handle by the popup should be moved to the current page's event handler
    page::handle_key_sequence_for_page(key_sequence, client_pub, state, ui)
}

/// Handle a command for a context list popup in which each item represents a context
///
/// # Arguments
/// In addition to application's states and the key sequence,
/// the function requires to specify:
/// - `uris`: a list of context URIs
/// - `uri_type`: an enum represents the type of a context in the list (`playlist`, `artist`, etc)
fn handle_command_for_context_browsing_list_popup(
    command: Command,
    ui: &mut UIStateGuard,
    uris: Vec<String>,
    context_type: rspotify_model::Type,
) -> Result<bool> {
    handle_command_for_list_popup(
        command,
        ui,
        uris.len(),
        |_, _| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            let uri = crate::utils::parse_uri(&uris[id]);
            let context_id = match context_type {
                rspotify_model::Type::Playlist => {
                    ContextId::Playlist(PlaylistId::from_uri(&uri)?.into_static())
                }
                rspotify_model::Type::Artist => {
                    ContextId::Artist(ArtistId::from_uri(&uri)?.into_static())
                }
                rspotify_model::Type::Album => {
                    ContextId::Album(AlbumId::from_uri(&uri)?.into_static())
                }
                _ => {
                    return Ok(());
                }
            };

            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });

            Ok(())
        },
        |ui: &mut UIStateGuard| {
            ui.popup = None;
        },
    )
}

/// Handle a command for a generic list popup.
///
/// # Arguments
/// - `n_items`: the number of items in the list
/// - `on_select_func`: the callback when selecting an item
/// - `on_choose_func`: the callback when choosing an item
/// - `on_close_func`: the callback when closing the popup
fn handle_command_for_list_popup(
    command: Command,
    ui: &mut UIStateGuard,
    n_items: usize,
    on_select_func: impl Fn(&mut UIStateGuard, usize),
    on_choose_func: impl Fn(&mut UIStateGuard, usize) -> Result<()>,
    on_close_func: impl Fn(&mut UIStateGuard),
) -> Result<bool> {
    let popup = ui.popup.as_mut().with_context(|| "expect a popup")?;
    let current_id = popup.list_selected().unwrap_or_default();

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
            if current_id < n_items {
                on_choose_func(ui, current_id)?;
            }
        }
        Command::ClosePopup => {
            on_close_func(ui);
        }
        _ => return Ok(false),
    };
    Ok(true)
}

fn handle_key_sequence_for_action_list_popup(
    n_actions: usize,
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let command = match config::get_config()
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => {
            // handle selecting an action by pressing a key from '0' to '9'
            if let Some(Key::None(crossterm::event::KeyCode::Char(c))) = key_sequence.keys.first() {
                if let Some(id) = c.to_digit(10) {
                    let id = id as usize;
                    if id < n_actions {
                        handle_item_action(id, client_pub, state, ui)?;
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }
    };

    handle_command_for_list_popup(
        command,
        ui,
        n_actions,
        |_, _| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            handle_item_action(id, client_pub, state, ui)
        },
        |ui: &mut UIStateGuard| {
            ui.popup = None;
        },
    )
}

/// Handle the `n`-th action in an action list popup
pub fn handle_item_action(
    n: usize,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<()> {
    let item = match ui.popup {
        Some(PopupState::ActionList(ref item, ..)) => item.clone(),
        _ => return Ok(()),
    };

    let data = state.data.read();

    match item {
        ActionListItem::Track(track, actions) => {
            handle_action_in_context(actions[n], track.into(), client_pub, &data, ui)
        }
        ActionListItem::Album(album, actions) => {
            handle_action_in_context(actions[n], album.into(), client_pub, &data, ui)
        }
        ActionListItem::Artist(artist, actions) => {
            handle_action_in_context(actions[n], artist.into(), client_pub, &data, ui)
        }
        ActionListItem::Playlist(playlist, actions) => {
            handle_action_in_context(actions[n], playlist.into(), client_pub, &data, ui)
        }
    }
}
