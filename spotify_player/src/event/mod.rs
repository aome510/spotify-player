use crate::{
    command::Command,
    key::{Key, KeySequence},
    state::*,
    utils::{new_list_state, new_table_state},
};
use anyhow::Result;
use tokio::sync::mpsc;

mod page;
mod popup;
mod window;

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
    TransferPlayback(String, bool),
    StartPlayback(Playback),
}

#[derive(Debug)]
/// A request to the client
pub enum ClientRequest {
    GetCurrentUser,
    GetDevices,
    GetUserPlaylists,
    GetUserSavedAlbums,
    GetUserFollowedArtists,
    GetUserTopTracks,
    GetUserRecentlyPlayedTracks,
    GetContext(ContextId),
    GetCurrentPlayback,
    GetRecommendations(SeedItem),
    Search(String),
    AddTrackToPlaylist(PlaylistId, TrackId),
    SaveToLibrary(Item),
    Player(PlayerRequest),
    GetLyric {
        track: String,
        artists: String,
    },
    #[cfg(feature = "streaming")]
    NewSpircConnection,
}

/// starts a terminal event handler (key pressed, mouse clicked, etc)
pub fn start_event_handler(state: SharedState, client_pub: mpsc::Sender<ClientRequest>) {
    while let Ok(event) = crossterm::event::read() {
        tracing::info!("got a terminal event: {:?}", event);

        if let Err(err) = match event {
            crossterm::event::Event::Mouse(event) => handle_mouse_event(event, &client_pub, &state),
            crossterm::event::Event::Key(event) => handle_key_event(event, &client_pub, &state),
            _ => Ok(()),
        } {
            tracing::error!("failed to handle event: {err:?}");
        }
    }
}

// handles a terminal mouse event
fn handle_mouse_event(
    event: crossterm::event::MouseEvent,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let ui = state.ui.lock();
    // a left click event
    if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = event.kind
    {
        if event.row == ui.progress_bar_rect.y {
            // calculate the seek position (in ms) based on the clicked position,
            // the progress bar's width and the track's duration (in ms)

            let player = state.player.read();
            let track = player.current_playing_track();
            if let Some(track) = track {
                let position_ms = (track.duration.as_millis() as u32) * (event.column as u32)
                    / (ui.progress_bar_rect.width as u32);
                client_pub
                    .blocking_send(ClientRequest::Player(PlayerRequest::SeekTrack(position_ms)))?;
            }
        }
    }
    Ok(())
}

// handle a terminal key pressed event
fn handle_key_event(
    event: crossterm::event::KeyEvent,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let key: Key = event.into();

    // parse the key sequence from user's previous inputs
    let mut key_sequence = state.ui.lock().input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let handled = if state.ui.lock().popup.is_none() {
        // no popup
        let page_type = state.ui.lock().current_page().page_type();
        match page_type {
            PageType::Library => page::handle_key_sequence_for_library_page(&key_sequence, state)?,
            PageType::Search => {
                page::handle_key_sequence_for_search_page(&key_sequence, client_pub, state)?
            }
            PageType::Context => {
                page::handle_key_sequence_for_context_page(&key_sequence, client_pub, state)?
            }
            PageType::Tracks => {
                page::handle_key_sequence_for_tracks_page(&key_sequence, client_pub, state)?
            }
            PageType::Lyric => {
                page::handle_key_sequence_for_lyric_page(&key_sequence, client_pub, state)?
            }
        }
    } else {
        popup::handle_key_sequence_for_popup(&key_sequence, client_pub, state)?
    };

    // if the key sequence is not handled, let the global command handler handle it
    let handled = if !handled {
        match state
            .keymap_config
            .find_command_from_key_sequence(&key_sequence)
        {
            Some(command) => handle_global_command(command, client_pub, state)?,
            None => false,
        }
    } else {
        true
    };

    // if successfully handled the key sequence, clear the key sequence.
    // else, the current key sequence is probably a prefix of a command's shortcut
    if handled {
        state.ui.lock().input_key_sequence.keys = vec![];
    } else {
        state.ui.lock().input_key_sequence = key_sequence;
    }
    Ok(())
}

/// handles a global command
fn handle_global_command(
    command: Command,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

    match command {
        Command::Quit => {
            ui.is_running = false;
        }
        Command::NextTrack => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::NextTrack))?;
        }
        Command::PreviousTrack => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::PreviousTrack))?;
        }
        Command::ResumePause => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::ResumePause))?;
        }
        Command::Repeat => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Repeat))?;
        }
        Command::Shuffle => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Shuffle))?;
        }
        Command::VolumeUp => {
            if let Some(ref playback) = state.player.read().playback {
                if let Some(percent) = playback.device.volume_percent {
                    let volume = std::cmp::min(percent + 5, 100_u32);
                    client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Volume(
                        volume as u8,
                    )))?;
                }
            }
        }
        Command::VolumeDown => {
            if let Some(ref playback) = state.player.read().playback {
                if let Some(percent) = playback.device.volume_percent {
                    let volume = std::cmp::max(percent.saturating_sub(5_u32), 0_u32);
                    client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Volume(
                        volume as u8,
                    )))?;
                }
            }
        }
        Command::OpenCommandHelp => {
            ui.popup = Some(PopupState::CommandHelp { scroll_offset: 0 });
        }
        Command::RefreshPlayback => {
            client_pub.blocking_send(ClientRequest::GetCurrentPlayback)?;
        }
        Command::ShowActionsOnCurrentTrack => {
            if let Some(track) = state.player.read().current_playing_track() {
                if let Some(track) = Track::try_from_full_track(track.clone()) {
                    ui.popup = Some(PopupState::ActionList(Item::Track(track), new_list_state()));
                }
            }
        }
        Command::CurrentlyPlayingContextPage => {
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::CurrentPlaying,
                state: None,
            });
        }
        Command::BrowseUserPlaylists => {
            client_pub.blocking_send(ClientRequest::GetUserPlaylists)?;
            ui.popup = Some(PopupState::UserPlaylistList(
                PlaylistPopupAction::Browse,
                new_list_state(),
            ));
        }
        Command::BrowseUserFollowedArtists => {
            client_pub.blocking_send(ClientRequest::GetUserFollowedArtists)?;
            ui.popup = Some(PopupState::UserFollowedArtistList(new_list_state()));
        }
        Command::BrowseUserSavedAlbums => {
            client_pub.blocking_send(ClientRequest::GetUserSavedAlbums)?;
            ui.popup = Some(PopupState::UserSavedAlbumList(new_list_state()));
        }
        Command::TopTrackPage => {
            ui.create_new_page(PageState::Tracks {
                id: "top-tracks".to_string(),
                title: "Top Tracks".to_string(),
                desc: "User's top tracks".to_string(),
                state: new_table_state(),
            });
            client_pub.blocking_send(ClientRequest::GetUserTopTracks)?;
        }
        Command::RecentlyPlayedTrackPage => {
            ui.create_new_page(PageState::Tracks {
                id: "recently-played-tracks".to_string(),
                title: "Recently Played Tracks".to_string(),
                desc: "User's recently played tracks".to_string(),
                state: new_table_state(),
            });
            client_pub.blocking_send(ClientRequest::GetUserRecentlyPlayedTracks)?;
        }
        Command::LibraryPage => {
            ui.create_new_page(PageState::Library {
                state: LibraryPageUIState::new(),
            });
            client_pub.blocking_send(ClientRequest::GetUserPlaylists)?;
            client_pub.blocking_send(ClientRequest::GetUserFollowedArtists)?;
            client_pub.blocking_send(ClientRequest::GetUserSavedAlbums)?;
        }
        Command::SearchPage => {
            ui.create_new_page(PageState::Search {
                input: String::new(),
                current_query: String::new(),
                state: SearchPageUIState::new(),
            });
        }
        Command::PreviousPage => {
            if ui.history.len() > 1 {
                ui.history.pop();
                ui.popup = None;
            }
        }
        Command::LyricPage => {
            if let Some(track) = state.player.read().current_playing_track() {
                let artists = track
                    .artists
                    .iter()
                    .map(|a| &a.name)
                    .fold(String::new(), |x, y| {
                        if x.is_empty() {
                            x + y
                        } else {
                            x + ", " + y
                        }
                    });
                ui.create_new_page(PageState::Lyric {
                    track: track.name.clone(),
                    artists: artists.clone(),
                    scroll_offset: 0,
                });

                client_pub.blocking_send(ClientRequest::GetLyric {
                    track: track.name.clone(),
                    artists,
                })?;
            }
        }
        Command::SwitchDevice => {
            ui.popup = Some(PopupState::DeviceList(new_list_state()));
            client_pub.blocking_send(ClientRequest::GetDevices)?;
        }
        Command::SwitchTheme => {
            // get the available themes with the current theme moved to the first position
            let mut themes = state.theme_config.themes.clone();
            let id = themes.iter().position(|t| t.name == ui.theme.name);
            if let Some(id) = id {
                let theme = themes.remove(id);
                themes.insert(0, theme);
            }

            ui.popup = Some(PopupState::ThemeList(themes, new_list_state()));
        }
        Command::ReconnectIntegratedClient => {
            #[cfg(feature = "streaming")]
            client_pub.blocking_send(ClientRequest::NewSpircConnection)?;
        }
        Command::FocusNextWindow => {
            if !ui.has_focused_popup() {
                ui.current_page_mut().next()
            }
        }
        Command::FocusPreviousWindow => {
            if !ui.has_focused_popup() {
                ui.current_page_mut().previous()
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
