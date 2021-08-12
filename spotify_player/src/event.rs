use crate::{
    command::Command,
    key::{Key, KeySequence},
    state::*,
};
use anyhow::Result;
use crossterm::event::{self, EventStream, KeyCode, KeyModifiers};
use rspotify::model::offset;
use std::sync::mpsc;
use tokio::stream::StreamExt;
use tui::widgets::ListState;

#[derive(Debug)]
pub enum ContextURI {
    Playlist(String),
    Album(String),
    Artist(String),
    Unknown(String),
}

#[derive(Debug)]
/// An event to communicate with the client
pub enum Event {
    GetDevices,
    GetCurrentPlayback,
    RefreshToken,
    NextTrack,
    PreviousTrack,
    ResumePause,
    SeekTrack(u32),
    Repeat,
    Shuffle,
    GetContext(ContextURI),
    PlayTrack(Option<String>, Option<Vec<String>>, Option<offset::Offset>),
    PlayContext(String),
    TransferPlayback(String),
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
/// starts the application's event stream that pools and handles events from the terminal
pub async fn start_event_stream(send: mpsc::Sender<Event>, state: SharedState) {
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
    send: &mpsc::Sender<Event>,
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
            if let PopupState::ContextSearch(_) = ui.popup_state {
                handle_key_sequence_for_search_popup(&key_sequence, send, state, &mut ui)?
            } else {
                false
            }
        }
        Some(command) => {
            // handle commands specifically for a popup window
            let handled = match ui.popup_state {
                PopupState::None => handle_command_for_none_popup(command, send, state, &mut ui)?,
                PopupState::ContextSearch(_) => {
                    handle_key_sequence_for_search_popup(&key_sequence, send, state, &mut ui)?
                }
                PopupState::ArtistList(_) => handle_command_for_generic_list_popup(
                    command,
                    match ui.popup_state {
                        PopupState::ArtistList(ref artists) => artists.len(),
                        _ => unreachable!(),
                    },
                    |ui: &mut UIStateGuard| ui.artists_list_ui_state.selected(),
                    |ui: &mut UIStateGuard, id: usize| {
                        ui.artists_list_ui_state.select(Some(id));
                    },
                    |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                        let artists = match ui.popup_state {
                            PopupState::ArtistList(ref artists) => artists,
                            _ => unreachable!(),
                        };
                        let uri = artists[id].uri.clone().unwrap();
                        send.send(Event::GetContext(ContextURI::Artist(uri.clone())))?;

                        let frame_state = FrameState::Browse(uri);
                        ui.frame_history.push(frame_state.clone());
                        ui.frame = frame_state;
                        ui.popup_state = PopupState::None;
                        Ok(())
                    },
                    |ui: &mut UIStateGuard| {
                        ui.popup_state = PopupState::None;
                    },
                    &mut ui,
                )?,
                PopupState::PlaylistList => {
                    let player = state.player.read().unwrap();
                    handle_command_for_generic_list_popup(
                        command,
                        player.user_playlists.len(),
                        |ui: &mut UIStateGuard| ui.playlists_list_ui_state.selected(),
                        |ui: &mut UIStateGuard, id: usize| {
                            ui.playlists_list_ui_state.select(Some(id));
                        },
                        |ui: &mut UIStateGuard, id: usize| -> Result<()> {
                            let uri = player.user_playlists[id].uri.clone();
                            send.send(Event::GetContext(ContextURI::Playlist(uri.clone())))?;

                            let frame_state = FrameState::Browse(uri);
                            ui.frame_history.push(frame_state.clone());
                            ui.frame = frame_state;
                            ui.popup_state = PopupState::None;
                            Ok(())
                        },
                        |ui: &mut UIStateGuard| {
                            ui.popup_state = PopupState::None;
                        },
                        &mut ui,
                    )?
                }
                PopupState::ThemeList(_) => handle_command_for_generic_list_popup(
                    command,
                    match ui.popup_state {
                        PopupState::ThemeList(ref themes) => themes.len(),
                        _ => unreachable!(),
                    },
                    |ui: &mut UIStateGuard| ui.themes_list_ui_state.selected(),
                    |ui: &mut UIStateGuard, id: usize| {
                        ui.theme = match ui.popup_state {
                            PopupState::ThemeList(ref themes) => themes[id].clone(),
                            _ => unreachable!(),
                        };
                        ui.themes_list_ui_state.select(Some(id));
                    },
                    |ui: &mut UIStateGuard, _: usize| -> Result<()> {
                        ui.popup_state = PopupState::None;
                        Ok(())
                    },
                    |ui: &mut UIStateGuard| {
                        ui.theme = match ui.popup_state {
                            PopupState::ThemeList(ref themes) => themes[0].clone(),
                            _ => unreachable!(),
                        };
                        ui.popup_state = PopupState::None;
                    },
                    &mut ui,
                )?,
                PopupState::DeviceList => {
                    let player = state.player.read().unwrap();

                    handle_command_for_generic_list_popup(
                        command,
                        player.devices.len(),
                        |ui: &mut UIStateGuard| ui.devices_list_ui_state.selected(),
                        |ui: &mut UIStateGuard, id: usize| {
                            ui.devices_list_ui_state.select(Some(id));
                        },
                        |_: &mut UIStateGuard, id: usize| -> Result<()> {
                            send.send(Event::TransferPlayback(player.devices[id].id.clone()))?;
                            Ok(())
                        },
                        |ui: &mut UIStateGuard| {
                            ui.popup_state = PopupState::None;
                        },
                        &mut ui,
                    )?
                }
                PopupState::CommandHelp => handle_command_for_command_help_popup(command, &mut ui)?,
            };

            if handled {
                true
            } else {
                handle_command(command, send, state, &mut ui)?
            }
        }
    };

    // if no command is handled, open the shortcuts help based on the current key sequence input
    if handled {
        ui.shortcuts_help_ui_state = false;
        ui.input_key_sequence.keys = vec![];
    } else {
        ui.shortcuts_help_ui_state = true;
        ui.input_key_sequence = key_sequence;
    }
    Ok(())
}

fn handle_mouse_event(
    event: event::MouseEvent,
    send: &mpsc::Sender<Event>,
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
                send.send(Event::SeekTrack(position_ms))?;
            }
        }
    }
    Ok(())
}

fn handle_command_for_none_popup(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::SearchContextTracks => {
            ui.context.select(Some(0));
            ui.popup_state = PopupState::ContextSearch("".to_owned());
            Ok(true)
        }
        Command::SortByTrack => {
            state
                .player
                .write()
                .unwrap()
                .context
                .sort_tracks(ContextSortOrder::TrackName);
            Ok(true)
        }
        Command::SortByAlbum => {
            state
                .player
                .write()
                .unwrap()
                .context
                .sort_tracks(ContextSortOrder::Album);
            Ok(true)
        }
        Command::SortByArtists => {
            state
                .player
                .write()
                .unwrap()
                .context
                .sort_tracks(ContextSortOrder::Artists);
            Ok(true)
        }
        Command::SortByAddedDate => {
            state
                .player
                .write()
                .unwrap()
                .context
                .sort_tracks(ContextSortOrder::AddedAt);
            Ok(true)
        }
        Command::SortByDuration => {
            state
                .player
                .write()
                .unwrap()
                .context
                .sort_tracks(ContextSortOrder::Duration);
            Ok(true)
        }
        Command::ReverseOrder => {
            state.player.write().unwrap().context.reverse_tracks();
            Ok(true)
        }
        Command::PlayContext => {
            let uri = state.player.read().unwrap().context.get_uri().to_owned();
            send.send(Event::PlayContext(uri))?;
            Ok(true)
        }
        Command::FocusNextWindow => {
            ui.context.next();
            Ok(true)
        }
        Command::FocusPreviousWindow => {
            ui.context.previous();
            Ok(true)
        }
        _ => handle_command_for_focused_context_window(command, send, ui, state),
    }
}

fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    if key_sequence.keys.len() == 1 {
        if let Key::None(c) = key_sequence.keys[0] {
            let query = match ui.popup_state {
                PopupState::ContextSearch(ref mut query) => query,
                _ => unreachable!(),
            };
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
                ui.context.select(Some(0));
                ui.popup_state = PopupState::None;
                Ok(true)
            }
            Command::FocusNextWindow => {
                ui.context.next();
                Ok(true)
            }
            Command::FocusPreviousWindow => {
                ui.context.previous();
                Ok(true)
            }
            _ => handle_command_for_focused_context_window(command, send, ui, state),
        },
        None => Ok(false),
    }
}

fn handle_command_for_generic_list_popup(
    command: Command,
    list_len: usize,
    get: impl Fn(&mut UIStateGuard) -> Option<usize>,
    set: impl Fn(&mut UIStateGuard, usize),
    choose_handle_func: impl Fn(&mut UIStateGuard, usize) -> Result<()>,
    close_handle_func: impl Fn(&mut UIStateGuard),
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::SelectNext => {
            if let Some(id) = get(ui) {
                if id + 1 < list_len {
                    set(ui, id + 1);
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = get(ui) {
                if id > 0 {
                    set(ui, id - 1);
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = get(ui) {
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

fn handle_command_for_command_help_popup(command: Command, ui: &mut UIStateGuard) -> Result<bool> {
    if let Command::ClosePopup = command {
        ui.popup_state = PopupState::None;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_command(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::Quit => {
            ui.is_running = false;
            Ok(true)
        }
        Command::NextTrack => {
            send.send(Event::NextTrack)?;
            Ok(true)
        }
        Command::PreviousTrack => {
            send.send(Event::PreviousTrack)?;
            Ok(true)
        }
        Command::ResumePause => {
            send.send(Event::ResumePause)?;
            Ok(true)
        }
        Command::Repeat => {
            send.send(Event::Repeat)?;
            Ok(true)
        }
        Command::Shuffle => {
            send.send(Event::Shuffle)?;
            Ok(true)
        }
        Command::OpenCommandHelp => {
            ui.popup_state = PopupState::CommandHelp;
            Ok(true)
        }
        Command::RefreshPlayback => {
            send.send(Event::GetCurrentPlayback)?;
            Ok(true)
        }
        Command::BrowsePlayingContext => {
            ui.frame = FrameState::Default;
            ui.frame_history.push(FrameState::Default);
            Ok(true)
        }
        Command::BrowsePlayingTrackAlbum => {
            if let Some(track) = state.player.read().unwrap().get_current_playing_track() {
                if let Some(ref uri) = track.album.uri {
                    send.send(Event::GetContext(ContextURI::Album(uri.clone())))?;
                    let frame_state = FrameState::Browse(uri.clone());
                    ui.frame_history.push(frame_state.clone());
                    ui.frame = frame_state;
                }
            }
            Ok(true)
        }
        Command::BrowsePlayingTrackArtist => {
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
                ui.popup_state = PopupState::ArtistList(artists);
                ui.artists_list_ui_state = ListState::default();
                ui.artists_list_ui_state.select(Some(0));
            }
            Ok(true)
        }
        Command::BrowseUserPlaylist => {
            ui.popup_state = PopupState::PlaylistList;
            ui.playlists_list_ui_state = ListState::default();
            ui.playlists_list_ui_state.select(Some(0));
            Ok(true)
        }
        Command::PreviousFrame => {
            if ui.frame_history.len() > 1 {
                ui.frame_history.pop();
                ui.frame = ui.frame_history.last().unwrap().clone();
            }
            Ok(true)
        }
        Command::SwitchDevice => {
            ui.popup_state = PopupState::DeviceList;
            ui.devices_list_ui_state = ListState::default();
            ui.devices_list_ui_state.select(Some(0));
            send.send(Event::GetDevices)?;
            Ok(true)
        }
        Command::SwitchTheme => {
            ui.popup_state = PopupState::ThemeList(state.get_themes(ui));
            ui.themes_list_ui_state = ListState::default();
            ui.themes_list_ui_state.select(Some(0));
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_focused_context_window(
    command: Command,
    send: &mpsc::Sender<Event>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    if let ContextState::Artist(_, _, _, focus_state) = ui.context {
        let player = state.player.read().unwrap();
        let albums = match player.context {
            Context::Artist(_, _, ref albums, _) => albums,
            _ => unreachable!(),
        };
        match focus_state {
            ArtistFocusState::Albums => {
                return handle_command_for_album_list(command, send, ui, albums);
            }
            ArtistFocusState::RelatedArtists => {}
            ArtistFocusState::TopTracks => {}
        }
    }

    handle_command_for_track_table(command, send, ui, state)
}

fn handle_command_for_album_list(
    command: Command,
    send: &mpsc::Sender<Event>,
    ui: &mut UIStateGuard,
    albums: &[Album],
) -> Result<bool> {
    match command {
        Command::SelectNext => {
            if let Some(id) = ui.context.selected() {
                if id + 1 < albums.len() {
                    ui.context.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.context.selected() {
                if id > 0 {
                    ui.context.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.context.selected() {
                let uri = albums[id].uri.clone().unwrap();
                send.send(Event::GetContext(ContextURI::Album(uri.clone())))?;
                let frame_state = FrameState::Browse(uri);
                ui.frame_history.push(frame_state.clone());
                ui.frame = frame_state;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_track_table(
    command: Command,
    send: &mpsc::Sender<Event>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let player = state.player.read().unwrap();
    let tracks = ui.get_context_tracks(&player);

    match command {
        Command::SelectNext => {
            if let Some(id) = ui.context.selected() {
                if id + 1 < tracks.len() {
                    ui.context.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.context.selected() {
                if id > 0 {
                    ui.context.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChooseSelected => {
            if let Some(id) = ui.context.selected() {
                match player.context {
                    Context::Artist(_, _, _, _) => {
                        // cannot use artist context uri with a track uri
                        let tracks = tracks.iter().map(|t| t.uri.clone()).collect::<Vec<_>>();
                        send.send(Event::PlayTrack(
                            None,
                            Some(tracks),
                            offset::for_position(id as u32),
                        ))?;
                    }
                    Context::Playlist(ref playlist, _) => {
                        send.send(Event::PlayTrack(
                            Some(playlist.uri.clone()),
                            None,
                            offset::for_uri(tracks[id].uri.clone()),
                        ))?;
                    }
                    Context::Album(ref album, _) => {
                        send.send(Event::PlayTrack(
                            Some(album.uri.clone()),
                            None,
                            offset::for_uri(tracks[id].uri.clone()),
                        ))?;
                    }
                    Context::Unknown(_) => {}
                }
            }
            Ok(true)
        }
        Command::BrowseSelectedTrackAlbum => {
            if let Some(id) = ui.context.selected() {
                if let Some(ref uri) = tracks[id].album.uri {
                    send.send(Event::GetContext(ContextURI::Album(uri.clone())))?;
                    let frame_state = FrameState::Browse(uri.clone());
                    ui.frame_history.push(frame_state.clone());
                    ui.frame = frame_state;
                }
            }
            Ok(true)
        }
        Command::BrowseSelectedTrackArtist => {
            if let Some(id) = ui.context.selected() {
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
                ui.popup_state = PopupState::ArtistList(artists);
                ui.artists_list_ui_state = ListState::default();
                ui.artists_list_ui_state.select(Some(0));
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
