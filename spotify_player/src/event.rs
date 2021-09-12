use crate::{
    command::Command,
    key::{Key, KeySequence},
    state::*,
    utils::{self, new_list_state},
};
use anyhow::Result;
use crossterm::event::{self, EventStream, KeyCode, KeyModifiers};
use rand::Rng;
use rspotify::model::offset;
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
/// An event that modifies the player's playback
pub enum PlayerEvent {
    NextTrack,
    PreviousTrack,
    ResumePause,
    SeekTrack(u32),
    Repeat,
    Shuffle,
    Volume(u8),
    PlayTrack(Option<String>, Option<Vec<String>>, Option<offset::Offset>),
}

#[derive(Debug)]
/// An event to communicate with the client
/// TODO: renaming this enum (e.g to `ClientRequest`)
pub enum Event {
    RefreshToken,
    GetDevices,
    GetUserPlaylists,
    GetUserSavedAlbums,
    GetUserFollowedArtists,
    GetContext(ContextURI),
    GetCurrentPlayback,
    TransferPlayback(String, bool),
    Search(String),
    Player(PlayerEvent),
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
/// starts a terminal event handler
pub async fn start_event_handler(send: mpsc::Sender<Event>, state: SharedState) {
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
            if let PopupState::ContextSearch(_) = ui.popup {
                handle_key_sequence_for_search_popup(&key_sequence, send, state, &mut ui)?
            } else {
                false
            }
        }
        Some(command) => {
            // handle commands specifically for a popup window
            let handled = match ui.popup {
                PopupState::None => handle_command_for_none_popup(command, send, state, &mut ui)?,
                PopupState::ContextSearch(_) => {
                    handle_key_sequence_for_search_popup(&key_sequence, send, state, &mut ui)?
                }
                PopupState::ArtistList(_, _) => handle_command_for_list_popup(
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
                        send.send(Event::GetContext(ContextURI::Artist(uri.clone())))?;

                        let frame_state = PageState::Browsing(uri);
                        ui.history.push(frame_state.clone());
                        ui.page = frame_state;
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
                            send.send(Event::TransferPlayback(
                                player.devices[id].id.clone(),
                                true,
                            ))?;
                            ui.popup = PopupState::None;
                            Ok(())
                        },
                        |ui: &mut UIStateGuard| {
                            ui.popup = PopupState::None;
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

    if handled {
        ui.input_key_sequence.keys = vec![];
    } else {
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
                send.send(Event::Player(PlayerEvent::SeekTrack(position_ms)))?;
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
        Command::EnterSearchPage => {
            // TODO: handle the command properly
            let new_page = PageState::Searching("blackpink".to_owned());
            ui.history.push(new_page.clone());
            ui.page = new_page;
            ui.window = WindowState::Search(
                new_list_state(),
                new_list_state(),
                new_list_state(),
                new_list_state(),
                SearchFocusState::Tracks,
            );
            // needs to set `context_uri` to an empty string
            // because keeping the original `context_uri` will
            // prevent the context window from updating when going
            // back from a search window using `PreviousPage` command
            state.player.write().unwrap().context_uri = "".to_owned();

            send.send(Event::Search("blackpink".to_owned()))?;
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
        _ => match ui.page {
            PageState::Browsing(_) => handle_command_for_context_window(command, send, state, ui),
            PageState::CurrentPlaying => {
                handle_command_for_context_window(command, send, state, ui)
            }
            PageState::Searching(_) => handle_command_for_search_window(command, send, state, ui),
        },
    }
}

fn handle_command_for_context_window(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
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
                        Context::Artist(_, _, _, _) => None,
                        _ => {
                            let id = rand::thread_rng().gen_range(0..tracks.len());
                            offset::for_uri(tracks[id].uri.clone())
                        }
                    };
                    send.send(Event::Player(PlayerEvent::PlayTrack(
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
                match state.player.write().unwrap().get_context_mut() {
                    Some(context) => match command {
                        Command::SortTrackByTitle => {
                            context.sort_tracks(ContextSortOrder::TrackName);
                            true
                        }
                        Command::SortTrackByAlbum => {
                            context.sort_tracks(ContextSortOrder::Album);
                            true
                        }
                        Command::SortTrackByArtists => {
                            context.sort_tracks(ContextSortOrder::Artists);
                            true
                        }
                        Command::SortTrackByAddedDate => {
                            context.sort_tracks(ContextSortOrder::AddedAt);
                            true
                        }
                        Command::SortTrackByDuration => {
                            context.sort_tracks(ContextSortOrder::Duration);
                            true
                        }
                        Command::ReverseTrackOrder => {
                            context.reverse_tracks();
                            true
                        }

                        _ => false,
                    },
                    None => false,
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

fn handle_command_for_search_window(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    Ok(false)
}

fn handle_key_sequence_for_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    if key_sequence.keys.len() == 1 {
        if let Key::None(c) = key_sequence.keys[0] {
            let query = match ui.popup {
                PopupState::ContextSearch(ref mut query) => query,
                _ => unreachable!(),
            };
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
    send: &mpsc::Sender<Event>,
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
            send.send(Event::GetContext(context_uri))?;

            let new_page = PageState::Browsing(uris[id].clone());
            ui.history.push(new_page.clone());
            ui.page = new_page;
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

fn handle_command_for_command_help_popup(command: Command, ui: &mut UIStateGuard) -> Result<bool> {
    if let Command::ClosePopup = command {
        ui.popup = PopupState::None;
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
            send.send(Event::Player(PlayerEvent::NextTrack))?;
            Ok(true)
        }
        Command::PreviousTrack => {
            send.send(Event::Player(PlayerEvent::PreviousTrack))?;
            Ok(true)
        }
        Command::ResumePause => {
            send.send(Event::Player(PlayerEvent::ResumePause))?;
            Ok(true)
        }
        Command::Repeat => {
            send.send(Event::Player(PlayerEvent::Repeat))?;
            Ok(true)
        }
        Command::Shuffle => {
            send.send(Event::Player(PlayerEvent::Shuffle))?;
            Ok(true)
        }
        Command::VolumeUp => {
            if let Some(ref playback) = state.player.read().unwrap().playback {
                let volume = std::cmp::min(playback.device.volume_percent + 5, 100_u32);
                send.send(Event::Player(PlayerEvent::Volume(volume as u8)))?;
            }
            Ok(true)
        }
        Command::VolumeDown => {
            if let Some(ref playback) = state.player.read().unwrap().playback {
                let volume = std::cmp::max(playback.device.volume_percent as i32 - 5, 0_i32);
                send.send(Event::Player(PlayerEvent::Volume(volume as u8)))?;
            }
            Ok(true)
        }
        Command::OpenCommandHelp => {
            ui.popup = PopupState::CommandHelp;
            Ok(true)
        }
        Command::RefreshPlayback => {
            send.send(Event::GetCurrentPlayback)?;
            Ok(true)
        }
        Command::BrowsePlayingContext => {
            ui.page = PageState::CurrentPlaying;
            ui.history.push(PageState::CurrentPlaying);
            Ok(true)
        }
        Command::BrowsePlayingTrackAlbum => {
            if let Some(track) = state.player.read().unwrap().get_current_playing_track() {
                if let Some(ref uri) = track.album.uri {
                    send.send(Event::GetContext(ContextURI::Album(uri.clone())))?;
                    let new_page = PageState::Browsing(uri.clone());
                    ui.history.push(new_page.clone());
                    ui.page = new_page;
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
            send.send(Event::GetUserPlaylists)?;
            ui.popup = PopupState::UserPlaylistList(utils::new_list_state());
            Ok(true)
        }
        Command::BrowseUserFollowedArtists => {
            send.send(Event::GetUserFollowedArtists)?;
            ui.popup = PopupState::UserFollowedArtistList(utils::new_list_state());
            Ok(true)
        }
        Command::BrowseUserSavedAlbums => {
            send.send(Event::GetUserSavedAlbums)?;
            ui.popup = PopupState::UserSavedAlbumList(utils::new_list_state());
            Ok(true)
        }
        Command::PreviousPage => {
            if ui.history.len() > 1 {
                ui.history.pop();
                ui.page = ui.history.last().unwrap().clone();
            }
            Ok(true)
        }
        Command::SwitchDevice => {
            ui.popup = PopupState::DeviceList(utils::new_list_state());
            send.send(Event::GetDevices)?;
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
    send: &mpsc::Sender<Event>,
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

fn handle_command_for_artist_list(
    command: Command,
    send: &mpsc::Sender<Event>,
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
                send.send(Event::GetContext(ContextURI::Artist(uri.clone())))?;
                let new_page = PageState::Browsing(uri);
                ui.history.push(new_page.clone());
                ui.page = new_page;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_album_list(
    command: Command,
    send: &mpsc::Sender<Event>,
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
                send.send(Event::GetContext(ContextURI::Album(uri.clone())))?;
                let new_page = PageState::Browsing(uri);
                ui.history.push(new_page.clone());
                ui.page = new_page;
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
                    send.send(Event::Player(PlayerEvent::PlayTrack(
                        None,
                        track_uris,
                        offset::for_position(id as u32),
                    )))?;
                } else if context_uri.is_some() {
                    // play a track from a context, use URI offset for finding the track
                    send.send(Event::Player(PlayerEvent::PlayTrack(
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
                    send.send(Event::GetContext(ContextURI::Album(uri.clone())))?;
                    let new_page = PageState::Browsing(uri.clone());
                    ui.history.push(new_page.clone());
                    ui.page = new_page;
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
