use anyhow::Context as _;
use rand::Rng;

use super::*;

pub fn handle_key_sequence_for_library_page(
    key_sequence: &KeySequence,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let mut ui = state.ui.lock();
    match command {
        Command::Search => {
            ui.current_page_mut().select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
            Ok(true)
        }
        _ => {
            let data = state.data.read();
            let focus_state = match ui.current_page() {
                PageState::Library { state } => state.focus,
                _ => anyhow::bail!("expect a library page state"),
            };
            match focus_state {
                LibraryFocusState::Playlists => window::handle_command_for_playlist_list_window(
                    command,
                    ui.search_filtered_items(&data.user_data.playlists),
                    &data,
                    ui,
                ),
                LibraryFocusState::SavedAlbums => window::handle_command_for_album_list_window(
                    command,
                    ui.search_filtered_items(&data.user_data.saved_albums),
                    &data,
                    ui,
                ),
                LibraryFocusState::FollowedArtists => {
                    window::handle_command_for_artist_list_window(
                        command,
                        ui.search_filtered_items(&data.user_data.followed_artists),
                        &data,
                        ui,
                    )
                }
            }
        }
    }
}

pub fn handle_key_sequence_for_search_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

    let (focus_state, input, current_query) = match ui.current_page_mut() {
        PageState::Search {
            state,
            input,
            current_query,
        } => (state.focus, input, current_query),
        _ => anyhow::bail!("expect a search page"),
    };

    // handle user's input
    if let SearchFocusState::Input = focus_state {
        if key_sequence.keys.len() == 1 {
            if let Key::None(c) = key_sequence.keys[0] {
                match c {
                    crossterm::event::KeyCode::Char(c) => {
                        input.push(c);
                        return Ok(true);
                    }
                    crossterm::event::KeyCode::Backspace => {
                        if !input.is_empty() {
                            input.pop().unwrap();
                        }
                        return Ok(true);
                    }
                    crossterm::event::KeyCode::Enter => {
                        if !input.is_empty() {
                            *current_query = input.clone();
                            client_pub.send(ClientRequest::Search(input.clone()))?;
                        }
                        return Ok(true);
                    }
                    _ => {}
                }
            }
        }
        return Ok(false);
    }

    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let data = state.data.read();
    let search_results = data.caches.search.peek(current_query);

    match focus_state {
        SearchFocusState::Input => anyhow::bail!("user's search input should be handled before"),
        SearchFocusState::Tracks => {
            let tracks = search_results
                .map(|s| s.tracks.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_track_list_window(command, client_pub, tracks, &data, ui)
        }
        SearchFocusState::Artists => {
            let artists = search_results
                .map(|s| s.artists.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_artist_list_window(command, artists, &data, ui)
        }
        SearchFocusState::Albums => {
            let albums = search_results
                .map(|s| s.albums.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_album_list_window(command, albums, &data, ui)
        }
        SearchFocusState::Playlists => {
            let playlists = search_results
                .map(|s| s.playlists.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_playlist_list_window(command, playlists, &data, ui)
        }
    }
}

pub fn handle_key_sequence_for_context_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let context_id = match state.ui.lock().current_page() {
        PageState::Context { id, .. } => id.clone(),
        _ => anyhow::bail!("expect a context page"),
    };

    match command {
        Command::Search => {
            let mut ui = state.ui.lock();
            ui.current_page_mut().select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
        }
        Command::PlayRandom => {
            if let Some(context_id) = context_id {
                let data = state.data.read();

                // randomly play a track from the current context
                if let Some(context) = data.caches.context.peek(&context_id.uri()) {
                    let tracks = context.tracks();
                    let offset = match context {
                        // Spotify does not allow to manually specify `offset` for artist context
                        Context::Artist { .. } => None,
                        _ => {
                            if tracks.is_empty() {
                                None
                            } else {
                                let id = rand::thread_rng().gen_range(0..tracks.len());
                                Some(rspotify_model::Offset::for_uri(&tracks[id].id.uri()))
                            }
                        }
                    };

                    client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                        Playback::Context(context_id, offset),
                    )))?;
                }
            }
        }
        _ => {
            // handle sort/reverse tracks commands
            let order = match command {
                Command::SortTrackByTitle => Some(TrackOrder::TrackName),
                Command::SortTrackByAlbum => Some(TrackOrder::Album),
                Command::SortTrackByArtists => Some(TrackOrder::Artists),
                Command::SortTrackByAddedDate => Some(TrackOrder::AddedAt),
                Command::SortTrackByDuration => Some(TrackOrder::Duration),
                _ => None,
            };

            if let Some(order) = order {
                if let Some(context_id) = context_id {
                    let mut data = state.data.write();
                    if let Some(context) = data.caches.context.peek_mut(&context_id.uri()) {
                        context.sort_tracks(order);
                    }
                }
                return Ok(true);
            }
            if command == Command::ReverseTrackOrder {
                if let Some(context_id) = context_id {
                    let mut data = state.data.write();
                    if let Some(context) = data.caches.context.peek_mut(&context_id.uri()) {
                        context.reverse_tracks();
                    }
                }
                return Ok(true);
            }

            // the command hasn't been handled, assign the job to the focused window's handler
            return window::handle_command_for_focused_context_window(command, client_pub, state);
        }
    }
    Ok(true)
}

pub fn handle_key_sequence_for_tracks_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let mut ui = state.ui.lock();
    let data = state.data.read();

    let id = match ui.current_page() {
        PageState::Tracks { id, .. } => id,
        _ => anyhow::bail!("expect a tracks page"),
    };

    let tracks = data
        .get_tracks_by_id(id)
        .map(|tracks| ui.search_filtered_items(tracks))
        .unwrap_or_default();

    match command {
        Command::Search => {
            ui.current_page_mut().select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
            Ok(true)
        }
        Command::PlayRandom => {
            // randomly play a song from the list of recommendation tracks
            let offset = {
                let id = rand::thread_rng().gen_range(0..tracks.len());
                Some(rspotify_model::Offset::for_uri(&tracks[id].id.uri()))
            };
            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(tracks.iter().map(|t| t.id.clone()).collect(), offset),
            )))?;

            Ok(true)
        }
        _ => window::handle_command_for_track_table_window(
            command,
            client_pub,
            None,
            Some(tracks.iter().map(|t| &t.id).collect()),
            tracks,
            &data,
            ui,
        ),
    }
}

pub fn handle_key_sequence_for_browse_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let mut ui = state.ui.lock();
    let data = state.data.read();

    let len = match ui.current_page() {
        PageState::Browse { state } => match state {
            BrowsePageUIState::CategoryList { .. } => {
                ui.search_filtered_items(&data.browse.categories).len()
            }
            BrowsePageUIState::CategoryPlaylistList { category, .. } => data
                .browse
                .category_playlists
                .get(&category.id)
                .map(|v| ui.search_filtered_items(v).len())
                .unwrap_or_default(),
        },
        _ => anyhow::bail!("expect a browse page state"),
    };

    let page_state = ui.current_page_mut();
    let selected = page_state.selected().unwrap_or_default();
    if selected >= len {
        return Ok(false);
    }

    match command {
        Command::ChooseSelected => {
            match page_state {
                PageState::Browse { state } => match state {
                    BrowsePageUIState::CategoryList { .. } => {
                        let categories = ui.search_filtered_items(&data.browse.categories);
                        client_pub.send(ClientRequest::GetBrowseCategoryPlaylists(
                            categories[selected].clone(),
                        ))?;
                        ui.create_new_page(PageState::Browse {
                            state: BrowsePageUIState::CategoryPlaylistList {
                                category: categories[selected].clone(),
                                state: new_list_state(),
                            },
                        });
                    }
                    BrowsePageUIState::CategoryPlaylistList { category, .. } => {
                        let playlists =
                            data.browse
                                .category_playlists
                                .get(&category.id)
                                .context(format!(
                                    "expect to have playlists data for {category} category"
                                ))?;
                        let context_id = ContextId::Playlist(
                            ui.search_filtered_items(playlists)[selected].id.clone(),
                        );
                        ui.create_new_page(PageState::Context {
                            id: None,
                            context_page_type: ContextPageType::Browsing(context_id),
                            state: None,
                        });
                    }
                },
                _ => anyhow::bail!("expect a browse page state"),
            };
        }
        Command::SelectNextOrScrollDown => {
            if selected + 1 < len {
                page_state.select(selected + 1);
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if selected > 0 {
                page_state.select(selected - 1);
            }
        }
        Command::Search => {
            page_state.select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(feature = "lyric-finder")]
pub fn handle_key_sequence_for_lyric_page(
    key_sequence: &KeySequence,
    _client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let mut ui = state.ui.lock();
    let scroll_offset = match ui.current_page_mut() {
        PageState::Lyric {
            ref mut scroll_offset,
            ..
        } => scroll_offset,
        _ => anyhow::bail!("expect a lyric page"),
    };

    match command {
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
