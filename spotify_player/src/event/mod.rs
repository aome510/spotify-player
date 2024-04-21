use crate::{
    client::{ClientRequest, PlayerRequest},
    command::{self, Command},
    config,
    key::{Key, KeySequence},
    state::*,
    ui::single_line_input::LineInput,
    utils::new_list_state,
};

#[cfg(feature = "lyric-finder")]
use crate::utils::map_join;
use anyhow::{Context as _, Result};

mod clipboard;
mod page;
mod popup;
mod window;

/// Start a terminal event handler (key pressed, mouse clicked, etc)
pub fn start_event_handler(state: SharedState, client_pub: flume::Sender<ClientRequest>) {
    while let Ok(event) = crossterm::event::read() {
        let _enter = tracing::info_span!("terminal_event", event = ?event).entered();
        if let Err(err) = match event {
            crossterm::event::Event::Mouse(event) => handle_mouse_event(event, &client_pub, &state),
            crossterm::event::Event::Key(event) => {
                if event.kind == crossterm::event::KeyEventKind::Press {
                    // only handle key press event to avoid handling a key event multiple times
                    // context:
                    // - https://github.com/crossterm-rs/crossterm/issues/752
                    // - https://github.com/aome510/spotify-player/issues/136
                    handle_key_event(event, &client_pub, &state)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        } {
            tracing::error!("Failed to handle event: {err:#}");
        }
    }
}

// Handle a terminal mouse event
fn handle_mouse_event(
    event: crossterm::event::MouseEvent,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    // a left click event
    if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = event.kind
    {
        tracing::debug!("Handling mouse event: {event:?}");
        let rect = state.ui.lock().playback_progress_bar_rect;
        if event.row == rect.y {
            // calculate the seek position (in ms) based on the mouse click position,
            // the progress bar's width and the track's duration (in ms)
            let duration = state
                .player
                .read()
                .current_playing_track()
                .map(|t| t.duration);
            if let Some(duration) = duration {
                let position_ms =
                    (duration.num_milliseconds()) * (event.column as i64) / (rect.width as i64);
                client_pub.send(ClientRequest::Player(PlayerRequest::SeekTrack(
                    chrono::Duration::try_milliseconds(position_ms).unwrap(),
                )))?;
            }
        }
    }
    Ok(())
}

// Handle a terminal key pressed event
fn handle_key_event(
    event: crossterm::event::KeyEvent,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let key: Key = event.into();
    let mut ui = state.ui.lock();

    let mut key_sequence = ui.input_key_sequence.clone();
    key_sequence.keys.push(key);

    // check if the current key sequence matches any keymap's prefix
    // if not, reset the key sequence
    if state
        .configs
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    tracing::debug!("Handling key event: {event:?}, current key sequence: {key_sequence:?}");
    let handled = {
        if ui.popup.is_none() {
            page::handle_key_sequence_for_page(&key_sequence, client_pub, state, &mut ui)?
        } else {
            popup::handle_key_sequence_for_popup(&key_sequence, client_pub, state, &mut ui)?
        }
    };

    // if the key sequence is not handled, let the global command handler handle it
    let handled = if !handled {
        match state
            .configs
            .keymap_config
            .find_command_from_key_sequence(&key_sequence)
        {
            Some(command) => handle_global_command(command, client_pub, state, &mut ui)?,
            None => false,
        }
    } else {
        true
    };

    // if handled, clear the key sequence
    // otherwise, the current key sequence can be a prefix of a command's shortcut
    if handled {
        ui.input_key_sequence.keys = vec![];
    } else {
        ui.input_key_sequence = key_sequence;
    }
    Ok(())
}

/// Handle a global command that is not specific to any page/popup
fn handle_global_command(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::Quit => {
            ui.is_running = false;
        }
        Command::NextTrack => {
            client_pub.send(ClientRequest::Player(PlayerRequest::NextTrack))?;
        }
        Command::PreviousTrack => {
            client_pub.send(ClientRequest::Player(PlayerRequest::PreviousTrack))?;
        }
        Command::ResumePause => {
            client_pub.send(ClientRequest::Player(PlayerRequest::ResumePause))?;
        }
        Command::Repeat => {
            client_pub.send(ClientRequest::Player(PlayerRequest::Repeat))?;
        }
        Command::ToggleFakeTrackRepeatMode => {
            let mut player = state.player.write();
            if let Some(playback) = &mut player.buffered_playback {
                playback.fake_track_repeat_state = !playback.fake_track_repeat_state;
            }
        }
        Command::Shuffle => {
            client_pub.send(ClientRequest::Player(PlayerRequest::Shuffle))?;
        }
        Command::VolumeUp => {
            if let Some(ref playback) = state.player.read().buffered_playback {
                if let Some(volume) = playback.volume {
                    let volume = std::cmp::min(volume + 5, 100_u32);
                    client_pub.send(ClientRequest::Player(PlayerRequest::Volume(volume as u8)))?;
                }
            }
        }
        Command::VolumeDown => {
            if let Some(ref playback) = state.player.read().buffered_playback {
                if let Some(volume) = playback.volume {
                    let volume = volume.saturating_sub(5_u32);
                    client_pub.send(ClientRequest::Player(PlayerRequest::Volume(volume as u8)))?;
                }
            }
        }
        Command::Mute => {
            client_pub.send(ClientRequest::Player(PlayerRequest::ToggleMute))?;
        }
        Command::SeekForward => {
            if let Some(progress) = state.player.read().playback_progress() {
                client_pub.send(ClientRequest::Player(PlayerRequest::SeekTrack(
                    progress + chrono::Duration::try_seconds(5).unwrap(),
                )))?;
            }
        }
        Command::SeekBackward => {
            if let Some(progress) = state.player.read().playback_progress() {
                client_pub.send(ClientRequest::Player(PlayerRequest::SeekTrack(
                    std::cmp::max(
                        chrono::Duration::zero(),
                        progress - chrono::Duration::try_seconds(5).unwrap(),
                    ),
                )))?;
            }
        }
        Command::OpenCommandHelp => {
            ui.new_page(PageState::CommandHelp { scroll_offset: 0 });
        }
        Command::RefreshPlayback => {
            client_pub.send(ClientRequest::GetCurrentPlayback)?;
        }
        Command::ShowActionsOnCurrentTrack => {
            if let Some(track) = state.player.read().current_playing_track() {
                if let Some(track) = Track::try_from_full_track(track.clone()) {
                    let data = state.data.read();
                    let actions = command::construct_track_actions(&track, &data);
                    ui.popup = Some(PopupState::ActionList(
                        ActionListItem::Track(track, actions),
                        new_list_state(),
                    ));
                }
            }
        }
        Command::CurrentlyPlayingContextPage => {
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::CurrentPlaying,
                state: None,
            });
        }
        Command::BrowseUserPlaylists => {
            client_pub.send(ClientRequest::GetUserPlaylists)?;
            ui.popup = Some(PopupState::UserPlaylistList(
                PlaylistPopupAction::Browse,
                new_list_state(),
            ));
        }
        Command::BrowseUserFollowedArtists => {
            client_pub.send(ClientRequest::GetUserFollowedArtists)?;
            ui.popup = Some(PopupState::UserFollowedArtistList(new_list_state()));
        }
        Command::BrowseUserSavedAlbums => {
            client_pub.send(ClientRequest::GetUserSavedAlbums)?;
            ui.popup = Some(PopupState::UserSavedAlbumList(new_list_state()));
        }
        Command::TopTrackPage => {
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(ContextId::Tracks(
                    USER_TOP_TRACKS_ID.to_owned(),
                )),
                state: None,
            });
            client_pub.send(ClientRequest::GetUserTopTracks)?;
        }
        Command::RecentlyPlayedTrackPage => {
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(ContextId::Tracks(
                    USER_RECENTLY_PLAYED_TRACKS_ID.to_owned(),
                )),
                state: None,
            });
            client_pub.send(ClientRequest::GetUserRecentlyPlayedTracks)?;
        }
        Command::LikedTrackPage => {
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(ContextId::Tracks(
                    USER_LIKED_TRACKS_ID.to_owned(),
                )),
                state: None,
            });
            client_pub.send(ClientRequest::GetUserSavedTracks)?;
        }
        Command::LibraryPage => {
            ui.new_page(PageState::Library {
                state: LibraryPageUIState::new(),
            });
        }
        Command::SearchPage => {
            ui.new_page(PageState::Search {
                line_input: LineInput::default(),
                current_query: String::new(),
                state: SearchPageUIState::new(),
            });
        }
        Command::BrowsePage => {
            ui.new_page(PageState::Browse {
                state: BrowsePageUIState::CategoryList {
                    state: new_list_state(),
                },
            });
            client_pub.send(ClientRequest::GetBrowseCategories)?;
        }
        Command::PreviousPage => {
            if ui.history.len() > 1 {
                ui.history.pop();
                ui.popup = None;
            }
        }
        Command::OpenSpotifyLinkFromClipboard => {
            let content = get_clipboard_content(&state.configs.app_config.paste_command)
                .context("get clipboard's content")?;
            let re = regex::Regex::new(
                r"https://open.spotify.com/(?P<type>.*?)/(?P<id>[[:alnum:]]*).*",
            )?;
            if let Some(cap) = re.captures(&content) {
                let typ = cap.name("type").expect("valid capture").as_str();
                let id = cap.name("id").expect("valid capture").as_str();
                match typ {
                    // for track link, play the song
                    "track" => {
                        let id = TrackId::from_id(id)?.into_static();
                        client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                            Playback::URIs(vec![id], None),
                            None,
                        )))?;
                    }
                    // for playlist/artist/album link, go to the corresponding context page
                    "playlist" => {
                        let id = PlaylistId::from_id(id)?.into_static();
                        ui.new_page(PageState::Context {
                            id: None,
                            context_page_type: ContextPageType::Browsing(ContextId::Playlist(id)),
                            state: None,
                        });
                    }
                    "artist" => {
                        let id = ArtistId::from_id(id)?.into_static();
                        ui.new_page(PageState::Context {
                            id: None,
                            context_page_type: ContextPageType::Browsing(ContextId::Artist(id)),
                            state: None,
                        });
                    }
                    "album" => {
                        let id = AlbumId::from_id(id)?.into_static();
                        ui.new_page(PageState::Context {
                            id: None,
                            context_page_type: ContextPageType::Browsing(ContextId::Album(id)),
                            state: None,
                        });
                    }
                    e => anyhow::bail!("unsupported Spotify type {e}!"),
                }
            } else {
                tracing::warn!("clipboard's content ({content}) is not a valid Spotify link!");
            }
        }
        #[cfg(feature = "lyric-finder")]
        Command::LyricPage => {
            if let Some(track) = state.player.read().current_playing_track() {
                let artists = map_join(&track.artists, |a| &a.name, ", ");
                ui.new_page(PageState::Lyric {
                    track: track.name.clone(),
                    artists: artists.clone(),
                    scroll_offset: 0,
                });

                client_pub.send(ClientRequest::GetLyric {
                    track: track.name.clone(),
                    artists,
                })?;
            }
        }
        Command::SwitchDevice => {
            ui.popup = Some(PopupState::DeviceList(new_list_state()));
            client_pub.send(ClientRequest::GetDevices)?;
        }
        Command::SwitchTheme => {
            // get the available themes with the current theme moved to the first position
            let mut themes = state.configs.theme_config.themes.clone();
            let id = themes.iter().position(|t| t.name == ui.theme.name);
            if let Some(id) = id {
                let theme = themes.remove(id);
                themes.insert(0, theme);
            }

            ui.popup = Some(PopupState::ThemeList(themes, new_list_state()));
        }
        #[cfg(feature = "streaming")]
        Command::RestartIntegratedClient => {
            client_pub.send(ClientRequest::RestartIntegratedClient)?;
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
        Command::Queue => {
            ui.new_page(PageState::Queue { scroll_offset: 0 });
            client_pub.send(ClientRequest::GetCurrentUserQueue)?;
        }
        Command::CreatePlaylist => {
            ui.popup = Some(PopupState::PlaylistCreate {
                name: LineInput::default(),
                desc: LineInput::default(),
                current_field: PlaylistCreateCurrentField::Name,
            });
        }
        Command::ClosePopup => {
            ui.popup = None;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn get_clipboard_content(cmd: &config::Command) -> Result<String> {
    let output = std::process::Command::new(&cmd.command)
        .args(&cmd.args)
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}
