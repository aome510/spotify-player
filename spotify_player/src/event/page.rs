use anyhow::Context as _;

use super::*;

macro_rules! handle_navigation_commands_for_page {
    ($state:ident, $command:ident, $len:expr, $page:expr, $id:expr) => {
        match $command {
            Command::SelectNextOrScrollDown => {
                if $id + 1 < $len {
                    $page.select($id + 1);
                }
                return Ok(true);
            }
            Command::SelectPreviousOrScrollUp => {
                if $id > 0 {
                    $page.select($id - 1);
                }
                return Ok(true);
            }
            Command::PageSelectNextOrScrollDown => {
                $page.select(std::cmp::min(
                    $id + $state.configs.app_config.page_size_in_rows,
                    $len - 1,
                ));
                return Ok(true);
            }
            Command::PageSelectPreviousOrScrollUp => {
                $page.select($id.saturating_sub($state.configs.app_config.page_size_in_rows));
                return Ok(true);
            }
            Command::SelectLastOrScrollToBottom => {
                if $len > 0 {
                    $page.select($len - 1);
                }
            }
            Command::SelectFirstOrScrollToTop => {
                $page.select(0);
            }
            _ => {}
        }
    };
}
pub(super) use handle_navigation_commands_for_page;

pub fn handle_key_sequence_for_library_page(
    key_sequence: &KeySequence,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .configs
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
                    state,
                ),
                LibraryFocusState::SavedAlbums => window::handle_command_for_album_list_window(
                    command,
                    ui.search_filtered_items(&data.user_data.saved_albums),
                    &data,
                    ui,
                    state,
                ),
                LibraryFocusState::FollowedArtists => {
                    window::handle_command_for_artist_list_window(
                        command,
                        ui.search_filtered_items(&data.user_data.followed_artists),
                        &data,
                        ui,
                        state,
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
        .configs
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let data = state.data.read();
    let search_results = data.caches.search.get(current_query);

    match focus_state {
        SearchFocusState::Input => anyhow::bail!("user's search input should be handled before"),
        SearchFocusState::Tracks => {
            let tracks = search_results
                .map(|s| s.tracks.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_track_list_window(
                command, client_pub, tracks, &data, ui, state,
            )
        }
        SearchFocusState::Artists => {
            let artists = search_results
                .map(|s| s.artists.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_artist_list_window(command, artists, &data, ui, state)
        }
        SearchFocusState::Albums => {
            let albums = search_results
                .map(|s| s.albums.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_album_list_window(command, albums, &data, ui, state)
        }
        SearchFocusState::Playlists => {
            let playlists = search_results
                .map(|s| s.playlists.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_playlist_list_window(command, playlists, &data, ui, state)
        }
    }
}

pub fn handle_key_sequence_for_context_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .configs
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    match command {
        Command::Search => {
            let mut ui = state.ui.lock();
            ui.current_page_mut().select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
        }
        _ => {
            // the command hasn't been handled, assign the job to the focused window's handler
            return window::handle_command_for_focused_context_window(command, client_pub, state);
        }
    }
    Ok(true)
}

pub fn handle_key_sequence_for_browse_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let command = match state
        .configs
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

    handle_navigation_commands_for_page!(state, command, len, page_state, selected);
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
        .configs
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
        Command::PageSelectNextOrScrollDown => {
            *scroll_offset += state.configs.app_config.page_size_in_rows;
        }
        Command::PageSelectPreviousOrScrollUp => {
            *scroll_offset =
                scroll_offset.saturating_sub(state.configs.app_config.page_size_in_rows);
        }
        Command::SelectFirstOrScrollToTop => {
            *scroll_offset = 0;
        }
        // Don't know the number of rows of a lyric displayed in the page, so just use a "big" number.
        // The `scroll_offset` will be adjust accordingly in the page rendering function.
        Command::SelectLastOrScrollToBottom => {
            *scroll_offset = 1024;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
