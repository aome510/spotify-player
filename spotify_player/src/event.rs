use crate::{
    command::Command,
    key::{Key, KeySequence},
    state::*,
    utils::{self, new_list_state},
};
use anyhow::Result;
use crossterm::event::{self, EventStream, KeyCode, KeyModifiers};
use rand::Rng;
use rspotify::model::{offset, playlist};
use std::sync::mpsc;
use tokio_stream::StreamExt;

#[derive(Debug)]
pub enum ContextURI {
    Playlist(String),
    Album(String),
    Artist(String),
    Unknown(String),
}

#[derive(Debug)]
/// A request that modifies the player's playback
pub enum PlayerRequest {
    NextTrack,
    PreviousTrack,
    ResumePause,
    SeekTrack(u32),
    Repeat,
    Shuffle,
    Volume(u8),
    PlayTrack(Option<String>, Option<Vec<String>>, Option<offset::Offset>),
    TransferPlayback(String, bool),
}

#[derive(Debug)]
/// A request to the client
pub enum ClientRequest {
    GetDevices,
    GetUserPlaylists,
    GetUserSavedAlbums,
    GetUserFollowedArtists,
    GetContext(ContextURI),
    GetCurrentPlayback,
    Search(String),
    Player(PlayerRequest),
}

impl From<event::KeyEvent> for Key {
    fn from(event: event::KeyEvent) -> Self {
        match event.modifiers {
            KeyModifiers::NONE => Key::None(event.code),
            KeyModifiers::ALT => Key::Alt(event.code),
            KeyModifiers::CONTROL => Key::Ctrl(event.code),
            KeyModifiers::SHIFT => Key::None(event.code),
            _ => unreachable!(),
        }
    }
}

#[tokio::main]
/// starts a handler to handle terminal events (key pressed, mouse clicked, etc)
pub async fn start_event_handler(send: mpsc::Sender<ClientRequest>, state: SharedState) {
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                log::info!("got event: {:?}", event);
                if let Err(err) = handle_terminal_event(event, &send, &state) {
                    log::warn!("failed to handle event: {:#}", err);
                }
            }
            Err(err) => {
                log::warn!("failed to get event: {:#}", err);
            }
        }
    }
}

fn handle_terminal_event(
    event: event::Event,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let key: Key = match event {
        event::Event::Key(event) => event.into(),
        event::Event::Mouse(event) => {
            return handle_mouse_event(event, send, state);
        }
        _ => {
            return Ok(());
        }
    };

    let mut ui = state.ui.lock().unwrap();

    let mut key_sequence = ui.input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let command = state
        .keymap_config
        .find_command_from_key_sequence(&key_sequence);

    let handled = match command {
        None => {
            // handle input key sequence for searching window/popup
            if let PopupState::ContextSearch(_) = ui.popup {
                handle_key_sequence_for_search_popup(&key_sequence, send, state, &mut ui)?
            } else if let PageState::Searching(..) = ui.current_page() {
                handle_key_sequence_for_search_window(&key_sequence, send, state, &mut ui)?
            } else {
                false
            }
        }
        Some(command) => {
            // handle commands specifically for a popup window
            let handled = match ui.popup {
                PopupState::None => match ui.current_page() {
                    // no popup, handle command based on the current UI's `PageState`
                    PageState::Browsing(_) => {
                        handle_command_for_context_window(command, send, state, &mut ui)?
                    }
                    PageState::CurrentPlaying => {
                        handle_command_for_context_window(command, send, state, &mut ui)?
                    }
                    PageState::Searching(..) => {
                        handle_key_sequence_for_search_window(&key_sequence, send, state, &mut ui)?
                    }
                },
                PopupState::ContextSearch(_) => {
                    handle_key_sequence_for_search_popup(&key_sequence, send, state, &mut ui)?
                }
                PopupState::ArtistList(..) => handle_command_for_list_popup(
                    command,
                    match ui.popup {
                        PopupState::ArtistList(ref artists, _) => artists.len(),
                        _ => unreachable!(),
                    },
                    |_: &mut UIStateGuard, _: usize| {},
                    |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                        let artists = match ui.popup {
                            PopupState::ArtistList(ref artists, _) => artists,
                            _ => unreachable!(),
                        };
                        let uri = artists[id].uri.clone().unwrap();
                        send.send(ClientRequest::GetContext(ContextURI::Artist(uri.clone())))?;

                        ui.history.push(PageState::Browsing(uri));
                        ui.popup = PopupState::None;
                        Ok(())
                    },
                    |ui: &mut UIStateGuard| {
                        ui.popup = PopupState::None;
                    },
                    &mut ui,
                )?,
                PopupState::UserPlaylistList(_) => {
                    let player = state.player.read().unwrap();
                    let playlist_uris = player
                        .user_playlists
                        .iter()
                        .map(|p| p.uri.clone())
                        .collect::<Vec<_>>();
                    handle_command_for_uri_list_popup(
                        command,
                        send,
                        &mut ui,
                        playlist_uris,
                        ContextURI::Playlist("".to_owned()),
                    )?
                }
                PopupState::UserFollowedArtistList(_) => {
                    let player = state.player.read().unwrap();
                    let artist_uris = player
                        .user_followed_artists
                        .iter()
                        .map(|a| a.uri.clone().unwrap())
                        .collect::<Vec<_>>();
                    handle_command_for_uri_list_popup(
                        command,
                        send,
                        &mut ui,
                        artist_uris,
                        ContextURI::Artist("".to_owned()),
                    )?
                }
                PopupState::UserSavedAlbumList(_) => {
                    let player = state.player.read().unwrap();
                    let album_uris = player
                        .user_saved_albums
                        .iter()
                        .map(|a| a.uri.clone().unwrap())
                        .collect::<Vec<_>>();

                    handle_command_for_uri_list_popup(
                        command,
                        send,
                        &mut ui,
                        album_uris,
                        ContextURI::Album("".to_owned()),
                    )?
                }
                PopupState::ThemeList(_, _) => handle_command_for_list_popup(
                    command,
                    match ui.popup {
                        PopupState::ThemeList(ref themes, _) => themes.len(),
                        _ => unreachable!(),
                    },
                    |ui: &mut UIStateGuard, id: usize| {
                        ui.theme = match ui.popup {
                            PopupState::ThemeList(ref themes, _) => themes[id].clone(),
                            _ => unreachable!(),
                        };
                    },
                    |ui: &mut UIStateGuard, _: usize| -> Result<()> {
                        ui.popup = PopupState::None;
                        Ok(())
                    },
                    |ui: &mut UIStateGuard| {
                        ui.theme = match ui.popup {
                            PopupState::ThemeList(ref themes, _) => themes[0].clone(),
                            _ => unreachable!(),
                        };
                        ui.popup = PopupState::None;
                    },
                    &mut ui,
                )?,
                PopupState::DeviceList(_) => {
                    let player = state.player.read().unwrap();

                    handle_command_for_list_popup(
                        command,
                        player.devices.len(),
                        |_: &mut UIStateGuard, _: usize| {},
                        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                            send.send(ClientRequest::Player(PlayerRequest::TransferPlayback(
                                player.devices[id].id.clone(),
                                true,
                            )))?;
                            ui.popup = PopupState::None;
                            Ok(())
                        },
                        |ui: &mut UIStateGuard| {
                            ui.popup = PopupState::None;
                        },
                        &mut ui,
                    )?
                }
                PopupState::CommandHelp(offset) => {
                    handle_command_for_command_help_popup(command, &mut ui, offset)?
                }
            };

            if handled {
                true
            } else {
                handle_command(command, send, state, &mut ui)?
            }
        }
    };

    if handled {
        ui.input_key_sequence.keys = vec![];
    } else {
        ui.input_key_sequence = key_sequence;
    }
    Ok(())
}

fn handle_mouse_event(
    event: event::MouseEvent,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let ui = state.ui.lock().unwrap();
    // a left click event
    if let event::MouseEventKind::Down(event::MouseButton::Left) = event.kind {
        if event.row == ui.progress_bar_rect.y {
            let player = state.player.read().unwrap();
            let track = player.get_current_playing_track();
            if let Some(track) = track {
                let position_ms =
                    track.duration_ms * (event.column as u32) / (ui.progress_bar_rect.width as u32);
                send.send(ClientRequest::Player(PlayerRequest::SeekTrack(position_ms)))?;
            }
        }
    }
    Ok(())
}

fn handle_command_for_context_window(
    command: Command,
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

fn handle_key_sequence_for_search_window(
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

fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let query = match ui.popup {
        PopupState::ContextSearch(ref mut query) => query,
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
                ui.popup = PopupState::None;
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
            _ => handle_command_for_focused_context_subwindow(command, send, ui, state),
        },
        None => Ok(false),
    }
}

fn handle_command_for_uri_list_popup(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    uris: Vec<String>,
    base_uri: ContextURI,
) -> Result<bool> {
    handle_command_for_list_popup(
        command,
        uris.len(),
        |_: &mut UIStateGuard, _: usize| {},
        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
            let uri = uris[id].clone();
            let context_uri = match base_uri {
                ContextURI::Playlist(_) => ContextURI::Playlist(uri),
                ContextURI::Artist(_) => ContextURI::Artist(uri),
                ContextURI::Album(_) => ContextURI::Album(uri),
                ContextURI::Unknown(_) => ContextURI::Unknown(uri),
            };
            send.send(ClientRequest::GetContext(context_uri))?;

            ui.history.push(PageState::Browsing(uris[id].clone()));
            ui.popup = PopupState::None;
            Ok(())
        },
        |ui: &mut UIStateGuard| {
            ui.popup = PopupState::None;
        },
        ui,
    )
}

fn handle_command_for_list_popup(
    command: Command,
    list_len: usize,
    on_select_func: impl Fn(&mut UIStateGuard, usize),
    choose_handle_func: impl Fn(&mut UIStateGuard, usize) -> Result<()>,
    close_handle_func: impl Fn(&mut UIStateGuard),
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::SelectNext => {
            if let Some(id) = ui.popup.list_selected() {
                if id + 1 < list_len {
                    ui.popup.list_select(Some(id + 1));
                    on_select_func(ui, id + 1);
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.popup.list_selected() {
                if id > 0 {
                    ui.popup.list_select(Some(id - 1));
                    on_select_func(ui, id - 1);
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.popup.list_selected() {
                choose_handle_func(ui, id)?;
            }
            Ok(true)
        }
        Command::ClosePopup => {
            close_handle_func(ui);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_command_help_popup(
    command: Command,
    ui: &mut UIStateGuard,
    page_offset: usize,
) -> Result<bool> {
    if let Command::ClosePopup = command {
        ui.popup = PopupState::None;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_command(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::Quit => {
            ui.is_running = false;
            Ok(true)
        }
        Command::NextTrack => {
            send.send(ClientRequest::Player(PlayerRequest::NextTrack))?;
            Ok(true)
        }
        Command::PreviousTrack => {
            send.send(ClientRequest::Player(PlayerRequest::PreviousTrack))?;
            Ok(true)
        }
        Command::ResumePause => {
            send.send(ClientRequest::Player(PlayerRequest::ResumePause))?;
            Ok(true)
        }
        Command::Repeat => {
            send.send(ClientRequest::Player(PlayerRequest::Repeat))?;
            Ok(true)
        }
        Command::Shuffle => {
            send.send(ClientRequest::Player(PlayerRequest::Shuffle))?;
            Ok(true)
        }
        Command::VolumeUp => {
            if let Some(ref playback) = state.player.read().unwrap().playback {
                let volume = std::cmp::min(playback.device.volume_percent + 5, 100_u32);
                send.send(ClientRequest::Player(PlayerRequest::Volume(volume as u8)))?;
            }
            Ok(true)
        }
        Command::VolumeDown => {
            if let Some(ref playback) = state.player.read().unwrap().playback {
                let volume = std::cmp::max(playback.device.volume_percent as i32 - 5, 0_i32);
                send.send(ClientRequest::Player(PlayerRequest::Volume(volume as u8)))?;
            }
            Ok(true)
        }
        Command::OpenCommandHelp => {
            ui.popup = PopupState::CommandHelp(0);
            Ok(true)
        }
        Command::RefreshPlayback => {
            send.send(ClientRequest::GetCurrentPlayback)?;
            Ok(true)
        }
        Command::BrowsePlayingContext => {
            ui.history.push(PageState::CurrentPlaying);
            Ok(true)
        }
        Command::BrowsePlayingTrackAlbum => {
            if let Some(track) = state.player.read().unwrap().get_current_playing_track() {
                if let Some(ref uri) = track.album.uri {
                    send.send(ClientRequest::GetContext(ContextURI::Album(uri.clone())))?;
                    ui.history.push(PageState::Browsing(uri.clone()));
                }
            }
            Ok(true)
        }
        Command::BrowsePlayingTrackArtists => {
            if let Some(track) = state.player.read().unwrap().get_current_playing_track() {
                let artists = track
                    .artists
                    .iter()
                    .map(|a| Artist {
                        name: a.name.clone(),
                        uri: a.uri.clone(),
                        id: a.id.clone(),
                    })
                    .filter(|a| a.uri.is_some())
                    .collect::<Vec<_>>();
                ui.popup = PopupState::ArtistList(artists, utils::new_list_state());
            }
            Ok(true)
        }
        Command::BrowseUserPlaylists => {
            send.send(ClientRequest::GetUserPlaylists)?;
            ui.popup = PopupState::UserPlaylistList(utils::new_list_state());
            Ok(true)
        }
        Command::BrowseUserFollowedArtists => {
            send.send(ClientRequest::GetUserFollowedArtists)?;
            ui.popup = PopupState::UserFollowedArtistList(utils::new_list_state());
            Ok(true)
        }
        Command::BrowseUserSavedAlbums => {
            send.send(ClientRequest::GetUserSavedAlbums)?;
            ui.popup = PopupState::UserSavedAlbumList(utils::new_list_state());
            Ok(true)
        }
        Command::PreviousPage => {
            if ui.history.len() > 1 {
                ui.history.pop();
            }
            Ok(true)
        }
        Command::SwitchDevice => {
            ui.popup = PopupState::DeviceList(utils::new_list_state());
            send.send(ClientRequest::GetDevices)?;
            Ok(true)
        }
        Command::SwitchTheme => {
            ui.popup = PopupState::ThemeList(state.get_themes(ui), utils::new_list_state());
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_focused_context_subwindow(
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
        Command::SelectNext => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < tracks.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
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
        Command::SelectNext => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < artists.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
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
        Command::SelectNext => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < albums.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
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
        Command::SelectNext => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < playlists.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
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
        Command::SelectNext => {
            if let Some(id) = ui.window.selected() {
                if id + 1 < tracks.len() {
                    ui.window.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
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
                ui.popup = PopupState::ArtistList(artists, utils::new_list_state());
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
