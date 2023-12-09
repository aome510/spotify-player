use crate::{
    command::{self, Command},
    key::{Key, KeySequence},
    state::*,
    utils::new_list_state,
};

#[cfg(feature = "lyric-finder")]
use crate::utils::map_join;
#[cfg(feature = "clipboard")]
use anyhow::Context as _;
use anyhow::Result;
#[cfg(feature = "clipboard")]
use copypasta::{ClipboardContext, ClipboardProvider};

mod page;
mod popup;
mod window;

#[derive(Debug)]
/// A request that modifies the player's playback
pub enum PlayerRequest {
    NextTrack,
    PreviousTrack,
    Resume,
    Pause,
    ResumePause,
    SeekTrack(chrono::Duration),
    Repeat,
    Shuffle,
    Volume(u8),
    ToggleMute,
    TransferPlayback(String, bool),
    StartPlayback(Playback, Option<bool>),
}

#[derive(Debug)]
/// A request to the client
pub enum ClientRequest {
    GetCurrentUser,
    GetDevices,
    GetBrowseCategories,
    GetBrowseCategoryPlaylists(Category),
    GetUserPlaylists,
    GetUserSavedAlbums,
    GetUserFollowedArtists,
    GetUserSavedTracks,
    GetUserTopTracks,
    GetUserRecentlyPlayedTracks,
    GetContext(ContextId),
    GetCurrentPlayback,
    GetRadioTracks {
        seed_uri: String,
        seed_name: String,
    },
    Search(String),
    AddTrackToQueue(TrackId<'static>),
    AddTrackToPlaylist(PlaylistId<'static>, TrackId<'static>),
    DeleteTrackFromPlaylist(PlaylistId<'static>, TrackId<'static>),
    ReorderPlaylistItems {
        playlist_id: PlaylistId<'static>,
        insert_index: usize,
        range_start: usize,
        range_length: Option<usize>,
        snapshot_id: Option<String>,
    },
    AddToLibrary(Item),
    DeleteFromLibrary(ItemId),
    ConnectDevice(Option<String>),
    Player(PlayerRequest),
    GetCurrentUserQueue,
    #[cfg(feature = "lyric-finder")]
    GetLyric {
        track: String,
        artists: String,
    },
    #[cfg(feature = "streaming")]
    NewStreamingConnection,
}

/// starts a terminal event handler (key pressed, mouse clicked, etc)
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

// handles a terminal mouse event
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
                    chrono::Duration::milliseconds(position_ms),
                )))?;
            }
        }
    }
    Ok(())
}

// handle a terminal key pressed event
fn handle_key_event(
    event: crossterm::event::KeyEvent,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let key: Key = event.into();

    // parse the key sequence from user's previous inputs
    let mut key_sequence = state.ui.lock().input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .configs
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    tracing::debug!("Handling key event: {event:?}, current key sequence: {key_sequence:?}");

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
            PageType::Browse => {
                page::handle_key_sequence_for_browse_page(&key_sequence, client_pub, state)?
            }
            #[cfg(feature = "lyric-finder")]
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
            .configs
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
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

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
                    progress + chrono::Duration::seconds(5),
                )))?;
            }
        }
        Command::SeekBackward => {
            if let Some(progress) = state.player.read().playback_progress() {
                client_pub.send(ClientRequest::Player(PlayerRequest::SeekTrack(
                    std::cmp::max(
                        chrono::Duration::zero(),
                        progress - chrono::Duration::seconds(5),
                    ),
                )))?;
            }
        }
        Command::OpenCommandHelp => {
            ui.popup = Some(PopupState::CommandHelp { scroll_offset: 0 });
        }
        Command::RefreshPlayback => {
            client_pub.send(ClientRequest::GetCurrentPlayback)?;
            // this will also reset the buffered playback
            state.player.write().buffered_playback = None;
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
            ui.create_new_page(PageState::Context {
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
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(ContextId::Tracks(
                    USER_TOP_TRACKS_ID.to_owned(),
                )),
                state: None,
            });
            client_pub.send(ClientRequest::GetUserTopTracks)?;
        }
        Command::RecentlyPlayedTrackPage => {
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(ContextId::Tracks(
                    USER_RECENTLY_PLAYED_TRACKS_ID.to_owned(),
                )),
                state: None,
            });
            client_pub.send(ClientRequest::GetUserRecentlyPlayedTracks)?;
        }
        Command::LikedTrackPage => {
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(ContextId::Tracks(
                    USER_LIKED_TRACKS_ID.to_owned(),
                )),
                state: None,
            });
            client_pub.send(ClientRequest::GetUserSavedTracks)?;
        }
        Command::LibraryPage => {
            ui.create_new_page(PageState::Library {
                state: LibraryPageUIState::new(),
            });
        }
        Command::SearchPage => {
            ui.create_new_page(PageState::Search {
                input: String::new(),
                current_query: String::new(),
                state: SearchPageUIState::new(),
            });
        }
        Command::BrowsePage => {
            ui.create_new_page(PageState::Browse {
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
        #[cfg(feature = "clipboard")]
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
                        ui.create_new_page(PageState::Context {
                            id: None,
                            context_page_type: ContextPageType::Browsing(ContextId::Playlist(id)),
                            state: None,
                        });
                    }
                    "artist" => {
                        let id = ArtistId::from_id(id)?.into_static();
                        ui.create_new_page(PageState::Context {
                            id: None,
                            context_page_type: ContextPageType::Browsing(ContextId::Artist(id)),
                            state: None,
                        });
                    }
                    "album" => {
                        let id = AlbumId::from_id(id)?.into_static();
                        ui.create_new_page(PageState::Context {
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
                ui.create_new_page(PageState::Lyric {
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
            client_pub.send(ClientRequest::NewStreamingConnection)?;
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
            ui.popup = Some(PopupState::Queue { scroll_offset: 0 });
            client_pub.send(ClientRequest::GetCurrentUserQueue)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(feature = "clipboard")]
fn get_clipboard_content() -> Result<String> {
    let mut clipboard_ctx = match ClipboardContext::new() {
        Ok(ctx) => ctx,
        Err(err) => anyhow::bail!("{err:#}"),
    };
    let content = match clipboard_ctx.get_contents() {
        Ok(content) => content,
        Err(err) => anyhow::bail!("{err:#}"),
    };
    Ok(content)
}
