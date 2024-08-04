use crate::{
    client::{ClientRequest, PlayerRequest},
    command::{self, construct_artist_actions, Action, ActionContext, Command},
    config,
    key::{Key, KeySequence},
    state::*,
    ui::single_line_input::LineInput,
    utils::parse_uri,
};

#[cfg(feature = "lyric-finder")]
use crate::utils::map_join;
use anyhow::{Context as _, Result};

use clipboard::{execute_copy_command, get_clipboard_content};
use tui::widgets::ListState;

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
    let keymap_config = &config::get_config().keymap_config;
    if !keymap_config.has_matched_prefix(&key_sequence) {
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
        match keymap_config.find_command_from_key_sequence(&key_sequence) {
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

pub fn handle_action_in_context(
    action: Action,
    context: ActionContext,
    client_pub: &flume::Sender<ClientRequest>,
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
) -> Result<()> {
    match context {
        ActionContext::Track(track) => match action {
            Action::GoToAlbum => {
                if let Some(album) = track.album {
                    let context_id = ContextId::Album(
                        AlbumId::from_uri(&parse_uri(&album.id.uri()))?.into_static(),
                    );
                    ui.new_page(PageState::Context {
                        id: None,
                        context_page_type: ContextPageType::Browsing(context_id),
                        state: None,
                    });
                }
            }
            Action::GoToArtist => {
                handle_go_to_artist(track.artists, ui);
            }
            Action::AddToQueue => {
                client_pub.send(ClientRequest::AddTrackToQueue(track.id))?;
                ui.popup = None;
            }
            Action::CopyLink => {
                let track_url = format!("https://open.spotify.com/track/{}", track.id.id());
                execute_copy_command(track_url)?;
                ui.popup = None;
            }
            Action::AddToPlaylist => {
                client_pub.send(ClientRequest::GetUserPlaylists)?;
                ui.popup = Some(PopupState::UserPlaylistList(
                    PlaylistPopupAction::AddTrack(track.id),
                    ListState::default(),
                ));
            }
            Action::ToggleLiked => {
                if data.user_data.is_liked_track(&track) {
                    client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Track(track.id)))?;
                } else {
                    client_pub.send(ClientRequest::AddToLibrary(Item::Track(track)))?;
                }
                ui.popup = None;
            }
            Action::AddToLiked => {
                client_pub.send(ClientRequest::AddToLibrary(Item::Track(track)))?;
                ui.popup = None;
            }
            Action::DeleteFromLiked => {
                client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Track(track.id)))?;
                ui.popup = None;
            }
            Action::GoToRadio => {
                let uri = track.id.uri();
                let name = track.name;
                ui.new_radio_page(&uri);
                client_pub.send(ClientRequest::GetRadioTracks {
                    seed_uri: uri,
                    seed_name: name,
                })?;
            }
            Action::ShowActionsOnArtist => handle_show_actions_on_artist(track.artists, data, ui),
            Action::ShowActionsOnAlbum => {
                if let Some(album) = track.album {
                    let context = ActionContext::Album(album.clone());
                    ui.popup = Some(PopupState::ActionList(
                        Box::new(ActionListItem::Album(
                            album,
                            context.get_available_actions(data),
                        )),
                        ListState::default(),
                    ));
                }
            }
            Action::DeleteFromPlaylist => {
                if let PageState::Context {
                    id: Some(ContextId::Playlist(playlist_id)),
                    ..
                } = ui.current_page()
                {
                    client_pub.send(ClientRequest::DeleteTrackFromPlaylist(
                        playlist_id.clone_static(),
                        track.id,
                    ))?;
                }
                ui.popup = None;
            }
            _ => {}
        },
        ActionContext::Album(album) => match action {
            Action::GoToArtist => {
                handle_go_to_artist(album.artists, ui);
            }
            Action::GoToRadio => {
                let uri = album.id.uri();
                let name = album.name;
                ui.new_radio_page(&uri);
                client_pub.send(ClientRequest::GetRadioTracks {
                    seed_uri: uri,
                    seed_name: name,
                })?;
            }
            Action::ShowActionsOnArtist => {
                handle_show_actions_on_artist(album.artists, data, ui);
            }
            Action::AddToLibrary => {
                client_pub.send(ClientRequest::AddToLibrary(Item::Album(album)))?;
                ui.popup = None;
            }
            Action::DeleteFromLibrary => {
                client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Album(album.id)))?;
                ui.popup = None;
            }
            Action::CopyLink => {
                let album_url = format!("https://open.spotify.com/album/{}", album.id.id());
                execute_copy_command(album_url)?;
                ui.popup = None;
            }
            Action::AddToQueue => {
                client_pub.send(ClientRequest::AddAlbumToQueue(album.id))?;
                ui.popup = None;
            }
            _ => {}
        },
        ActionContext::Artist(artist) => match action {
            Action::Follow => {
                client_pub.send(ClientRequest::AddToLibrary(Item::Artist(artist)))?;
                ui.popup = None;
            }
            Action::Unfollow => {
                client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Artist(artist.id)))?;
                ui.popup = None;
            }
            Action::CopyLink => {
                let artist_url = format!("https://open.spotify.com/artist/{}", artist.id.id());
                execute_copy_command(artist_url)?;
                ui.popup = None;
            }
            Action::GoToRadio => {
                let uri = artist.id.uri();
                let name = artist.name;
                ui.new_radio_page(&uri);
                client_pub.send(ClientRequest::GetRadioTracks {
                    seed_uri: uri,
                    seed_name: name,
                })?;
            }
            _ => {}
        },
        ActionContext::Playlist(playlist) => match action {
            Action::AddToLibrary => {
                client_pub.send(ClientRequest::AddToLibrary(Item::Playlist(playlist)))?;
                ui.popup = None;
            }
            Action::GoToRadio => {
                let uri = playlist.id.uri();
                let name = playlist.name;
                ui.new_radio_page(&uri);
                client_pub.send(ClientRequest::GetRadioTracks {
                    seed_uri: uri,
                    seed_name: name,
                })?;
            }
            Action::CopyLink => {
                let playlist_url =
                    format!("https://open.spotify.com/playlist/{}", playlist.id.id());
                execute_copy_command(playlist_url)?;
                ui.popup = None;
            }
            Action::DeleteFromLibrary => {
                client_pub.send(ClientRequest::DeleteFromLibrary(ItemId::Playlist(
                    playlist.id,
                )))?;
                ui.popup = None;
            }
            _ => {}
        },
    }

    Ok(())
}

fn handle_go_to_artist(artists: Vec<Artist>, ui: &mut UIStateGuard) {
    if artists.len() == 1 {
        let context_id = ContextId::Artist(artists[0].id.clone());
        ui.new_page(PageState::Context {
            id: None,
            context_page_type: ContextPageType::Browsing(context_id),
            state: None,
        });
    } else {
        ui.popup = Some(PopupState::ArtistList(
            ArtistPopupAction::Browse,
            artists,
            ListState::default(),
        ));
    }
}

fn handle_show_actions_on_artist(
    artists: Vec<Artist>,
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
) {
    if artists.len() == 1 {
        let actions = construct_artist_actions(&artists[0], data);
        ui.popup = Some(PopupState::ActionList(
            Box::new(ActionListItem::Artist(artists[0].clone(), actions)),
            ListState::default(),
        ));
    } else {
        ui.popup = Some(PopupState::ArtistList(
            ArtistPopupAction::ShowActions,
            artists,
            ListState::default(),
        ));
    }
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
                let duration = config::get_config().app_config.seek_duration_secs;
                client_pub.send(ClientRequest::Player(PlayerRequest::SeekTrack(
                    progress + chrono::Duration::try_seconds(duration as i64).unwrap(),
                )))?;
            }
        }
        Command::SeekBackward => {
            if let Some(progress) = state.player.read().playback_progress() {
                let duration = config::get_config().app_config.seek_duration_secs;
                client_pub.send(ClientRequest::Player(PlayerRequest::SeekTrack(
                    std::cmp::max(
                        chrono::Duration::zero(),
                        progress - chrono::Duration::try_seconds(duration as i64).unwrap(),
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
                        Box::new(ActionListItem::Track(track, actions)),
                        ListState::default(),
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
                ListState::default(),
            ));
        }
        Command::BrowseUserFollowedArtists => {
            client_pub.send(ClientRequest::GetUserFollowedArtists)?;
            ui.popup = Some(PopupState::UserFollowedArtistList(ListState::default()));
        }
        Command::BrowseUserSavedAlbums => {
            client_pub.send(ClientRequest::GetUserSavedAlbums)?;
            ui.popup = Some(PopupState::UserSavedAlbumList(ListState::default()));
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
                    state: ListState::default(),
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
            let content = get_clipboard_content().context("get clipboard's content")?;
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
            ui.popup = Some(PopupState::DeviceList(ListState::default()));
            client_pub.send(ClientRequest::GetDevices)?;
        }
        Command::SwitchTheme => {
            // get the available themes with the current theme moved to the first position
            let mut themes = config::get_config().theme_config.themes.clone();
            let id = themes.iter().position(|t| t.name == ui.theme.name);
            if let Some(id) = id {
                let theme = themes.remove(id);
                themes.insert(0, theme);
            }

            ui.popup = Some(PopupState::ThemeList(themes, ListState::default()));
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
        Command::JumpToCurrentTrackInContext => {
            let track_id = match state
                .player
                .read()
                .current_playing_track()
                .and_then(|track| track.id.clone())
            {
                Some(id) => id,
                None => return Ok(false),
            };

            if let PageState::Context {
                id: Some(context_id),
                ..
            } = ui.current_page()
            {
                let context_track_pos = state
                    .data
                    .read()
                    .context_tracks(context_id)
                    .and_then(|tracks| tracks.iter().position(|t| t.id == track_id));

                if let Some(p) = context_track_pos {
                    ui.current_page_mut().select(p);
                }
            }
        }
        Command::ClosePopup => {
            ui.popup = None;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
