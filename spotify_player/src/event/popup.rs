use super::*;
use crate::command::{AlbumAction, ArtistAction, PlaylistAction, TrackAction};
use anyhow::Context;

/// handles a key sequence for a popup
pub fn handle_key_sequence_for_popup(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let ui = state.ui.lock();
    let popup = ui
        .popup
        .as_ref()
        .with_context(|| "expect a popup".to_string())?;

    if let PopupState::Search { .. } = popup {
        drop(ui);
        return handle_key_sequence_for_search_popup(key_sequence, client_pub, state);
    }

    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    match popup {
        PopupState::Search { .. } => anyhow::bail!("should be handled before"),
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
                            ui.create_new_page(PageState::Context {
                                id: None,
                                context_page_type: ContextPageType::Browsing(context_id),
                                state: None,
                            });
                        }
                        ArtistPopupAction::GoToRadio => {
                            let uri = artists[id].id.uri();
                            let desc = format!("{} Radio", artists[id].name);
                            ui.create_new_radio_page(&uri, desc);
                            client_pub.send(ClientRequest::GetRadioTracks(uri))?;
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
        PopupState::CommandHelp { .. } => handle_command_for_command_help_popup(command, ui),
        PopupState::ActionList(item, ..) => {
            handle_command_for_action_list_popup(item.n_actions(), command, client_pub, ui)
        }
    }
}

/// handles a key sequence for a context search popup
fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    // handle user's input that updates the search query
    {
        let mut ui = state.ui.lock();
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
    if let Some(Command::ClosePopup) = command {
        state.ui.lock().popup = None;
        return Ok(true);
    }

    // there is no focus placed on the search popup, so commands not handle by
    // the popup should be moved to the current page's event handler
    let page_type = state.ui.lock().current_page().page_type();
    match page_type {
        PageType::Library => page::handle_key_sequence_for_library_page(key_sequence, state),
        PageType::Search => {
            page::handle_key_sequence_for_search_page(key_sequence, client_pub, state)
        }
        PageType::Context => {
            page::handle_key_sequence_for_context_page(key_sequence, client_pub, state)
        }
        PageType::Browse => {
            page::handle_key_sequence_for_browse_page(key_sequence, client_pub, state)
        }
        #[cfg(feature = "lyric-finder")]
        PageType::Lyric => {
            page::handle_key_sequence_for_lyric_page(key_sequence, client_pub, state)
        }
    }
}

/// Handles a command for a context list popup in which each item represents a context
///
/// In addition to application's states and the key sequence,
/// the function requires to specify:
/// - `uris`: a list of context URIs
/// - `uri_type`: an enum represents the type of a context in the list (`playlist`, `artist`, etc)
fn handle_command_for_context_browsing_list_popup(
    command: Command,
    ui: UIStateGuard,
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

            ui.create_new_page(PageState::Context {
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

/// Handles a command for a generic list popup.
///
/// - `n_items`: the number of items in the list
/// - `on_select_func`: the callback when selecting an item
/// - `on_choose_func`: the callback when choosing an item
/// - `on_close_func`: the callback when closing the popup
fn handle_command_for_list_popup(
    command: Command,
    mut ui: UIStateGuard,
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
        _ => return Ok(false),
    };
    Ok(true)
}

/// handles a command for a command shortcut help popup
fn handle_command_for_command_help_popup(command: Command, mut ui: UIStateGuard) -> Result<bool> {
    let scroll_offset = match ui.popup {
        Some(PopupState::CommandHelp {
            ref mut scroll_offset,
        }) => scroll_offset,
        _ => return Ok(false),
    };
    match command {
        Command::ClosePopup => {
            ui.popup = None;
        }
        Command::SelectNextOrScrollDown => {
            *scroll_offset += 1;
        }
        Command::SelectPreviousOrScrollUp => {
            if *scroll_offset > 0 {
                *scroll_offset -= 1;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// handles a command for an action list popup
fn handle_command_for_action_list_popup(
    n_actions: usize,
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    ui: UIStateGuard,
) -> Result<bool> {
    handle_command_for_list_popup(
        command,
        ui,
        n_actions,
        |_, _| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            let item = match ui.popup {
                Some(PopupState::ActionList(ref item, ..)) => item,
                _ => return Ok(()),
            };

            match item {
                ActionListItem::Track(track, actions) => match actions[id] {
                    TrackAction::GoToAlbum => {
                        if let Some(ref album) = track.album {
                            let uri = album.id.uri();
                            let context_id = ContextId::Album(
                                AlbumId::from_uri(&crate::utils::parse_uri(&uri))?.into_static(),
                            );
                            ui.create_new_page(PageState::Context {
                                id: None,
                                context_page_type: ContextPageType::Browsing(context_id),
                                state: None,
                            });
                        }
                    }
                    TrackAction::GoToArtist => {
                        ui.popup = Some(PopupState::ArtistList(
                            ArtistPopupAction::Browse,
                            track.artists.clone(),
                            new_list_state(),
                        ));
                    }
                    TrackAction::AddToQueue => {
                        client_pub.send(ClientRequest::AddTrackToQueue(track.id.clone()))?;
                        ui.popup = None;
                    }
                    TrackAction::AddToPlaylist => {
                        client_pub.send(ClientRequest::GetUserPlaylists)?;
                        ui.popup = Some(PopupState::UserPlaylistList(
                            PlaylistPopupAction::AddTrack(track.id.clone()),
                            new_list_state(),
                        ));
                    }
                    TrackAction::AddToLikedTracks => {
                        client_pub.send(ClientRequest::AddToLibrary(Item::Track(track.clone())))?;
                        ui.popup = None;
                    }
                    TrackAction::GoToTrackRadio => {
                        let uri = track.id.uri();
                        let desc = format!("{} Radio", track.name);
                        ui.create_new_radio_page(&uri, desc);
                        client_pub.send(ClientRequest::GetRadioTracks(uri))?;
                    }
                    TrackAction::GoToArtistRadio => {
                        ui.popup = Some(PopupState::ArtistList(
                            ArtistPopupAction::GoToRadio,
                            track.artists.clone(),
                            new_list_state(),
                        ));
                    }
                    TrackAction::GoToAlbumRadio => {
                        if let Some(ref album) = track.album {
                            let uri = album.id.uri();
                            let desc = format!("{} Radio", album.name);
                            ui.create_new_radio_page(&uri, desc);
                            client_pub.send(ClientRequest::GetRadioTracks(uri))?;
                        }
                    }
                    TrackAction::DeleteFromLikedTracks => {
                        client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Track(
                            track.id.clone(),
                        )))?;
                        ui.popup = None;
                    }
                    TrackAction::DeleteFromCurrentPlaylist => {
                        if let PageState::Context {
                            id: Some(ContextId::Playlist(playlist_id)),
                            ..
                        } = ui.current_page()
                        {
                            client_pub.send(ClientRequest::DeleteTrackFromPlaylist(
                                playlist_id.clone(),
                                track.id.clone(),
                            ))?;
                        }
                        ui.popup = None;
                    }
                },
                ActionListItem::Album(album, actions) => match actions[id] {
                    AlbumAction::GoToArtist => {
                        ui.popup = Some(PopupState::ArtistList(
                            ArtistPopupAction::Browse,
                            album.artists.clone(),
                            new_list_state(),
                        ));
                    }
                    AlbumAction::GoToAlbumRadio => {
                        let uri = album.id.uri();
                        let desc = format!("{} Radio", album.name);
                        ui.create_new_radio_page(&uri, desc);
                        client_pub.send(ClientRequest::GetRadioTracks(uri))?;
                    }
                    AlbumAction::GoToArtistRadio => {
                        ui.popup = Some(PopupState::ArtistList(
                            ArtistPopupAction::GoToRadio,
                            album.artists.clone(),
                            new_list_state(),
                        ));
                    }
                    AlbumAction::AddToLibrary => {
                        client_pub.send(ClientRequest::AddToLibrary(Item::Album(album.clone())))?;
                        ui.popup = None;
                    }
                    AlbumAction::DeleteFromLibrary => {
                        client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Album(
                            album.id.clone(),
                        )))?;
                        ui.popup = None;
                    }
                },
                ActionListItem::Artist(artist, actions) => match actions[id] {
                    ArtistAction::Follow => {
                        client_pub
                            .send(ClientRequest::AddToLibrary(Item::Artist(artist.clone())))?;
                        ui.popup = None;
                    }
                    ArtistAction::GoToArtistRadio => {
                        let uri = artist.id.uri();
                        let desc = format!("{} Radio", artist.name);
                        ui.create_new_radio_page(&uri, desc);
                        client_pub.send(ClientRequest::GetRadioTracks(uri))?;
                    }
                    ArtistAction::Unfollow => {
                        client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Artist(
                            artist.id.clone(),
                        )))?;
                        ui.popup = None;
                    }
                },
                ActionListItem::Playlist(playlist, actions) => match actions[id] {
                    PlaylistAction::AddToLibrary => {
                        client_pub.send(ClientRequest::AddToLibrary(Item::Playlist(
                            playlist.clone(),
                        )))?;
                        ui.popup = None;
                    }
                    PlaylistAction::GoToPlaylistRadio => {
                        let uri = playlist.id.uri();
                        let desc = format!("{} Radio", playlist.name);
                        ui.create_new_radio_page(&uri, desc);
                        client_pub.send(ClientRequest::GetRadioTracks(uri))?;
                    }
                    PlaylistAction::DeleteFromLibrary => {
                        client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Playlist(
                            playlist.id.clone(),
                        )))?;
                        ui.popup = None;
                    }
                },
            }
            Ok(())
        },
        |ui: &mut UIStateGuard| {
            ui.popup = None;
        },
    )
}
