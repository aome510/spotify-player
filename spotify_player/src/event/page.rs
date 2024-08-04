use anyhow::Context as _;
use command::CommandOrAction;

use super::*;

pub fn handle_key_sequence_for_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let page_type = ui.current_page().page_type();
    // handle search page separately as it needs access to the raw key sequence
    // as opposed to the matched command
    if page_type == PageType::Search {
        return handle_key_sequence_for_search_page(key_sequence, client_pub, state, ui);
    }

    match config::get_config()
        .keymap_config
        .find_command_or_action_from_key_sequence(key_sequence)
    {
        Some(CommandOrAction::Command(command)) => match page_type {
            PageType::Search => anyhow::bail!("page search type should already be handled!"),
            PageType::Library => handle_command_for_library_page(command, client_pub, ui, state),
            PageType::Context => handle_command_for_context_page(command, client_pub, ui, state),
            PageType::Browse => handle_command_for_browse_page(command, client_pub, ui, state),
            #[cfg(feature = "lyric-finder")]
            PageType::Lyric => handle_command_for_lyric_page(command, ui),
            PageType::Queue => handle_command_for_queue_page(command, ui),
            PageType::CommandHelp => handle_command_for_command_help_page(command, ui),
        },
        Some(CommandOrAction::Action(action)) => match page_type {
            PageType::Search => anyhow::bail!("page search type should already be handled!"),
            PageType::Library => handle_action_for_library_page(action, client_pub, ui, state),
            PageType::Context => {
                window::handle_action_for_focused_context_page(action, client_pub, ui, state)
            }
            PageType::Browse => handle_action_for_browse_page(action, client_pub, ui, state),
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_action_for_library_page(
    action: Action,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let data = state.data.read();
    let focus_state = match ui.current_page() {
        PageState::Library { state } => state.focus,
        _ => anyhow::bail!("expect a library page state"),
    };
    match focus_state {
        LibraryFocusState::Playlists => window::handle_action_for_selected_item(
            action,
            ui.search_filtered_items(&data.user_data.playlists),
            &data,
            ui,
            client_pub,
        ),
        LibraryFocusState::SavedAlbums => window::handle_action_for_selected_item(
            action,
            ui.search_filtered_items(&data.user_data.saved_albums),
            &data,
            ui,
            client_pub,
        ),
        LibraryFocusState::FollowedArtists => window::handle_action_for_selected_item(
            action,
            ui.search_filtered_items(&data.user_data.followed_artists),
            &data,
            ui,
            client_pub,
        ),
    }
}

fn handle_command_for_library_page(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    match command {
        Command::Search => {
            ui.new_search_popup();
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
                    client_pub,
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

fn handle_key_sequence_for_search_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let (focus_state, current_query, line_input) = match ui.current_page_mut() {
        PageState::Search {
            state,
            line_input,
            current_query,
        } => (state.focus, current_query, line_input),
        _ => anyhow::bail!("expect a search page"),
    };

    // handle user's input
    if let SearchFocusState::Input = focus_state {
        if key_sequence.keys.len() == 1 {
            return match &key_sequence.keys[0] {
                Key::None(crossterm::event::KeyCode::Enter) => {
                    if !line_input.is_empty() {
                        *current_query = line_input.get_text();
                        client_pub.send(ClientRequest::Search(line_input.get_text()))?;
                    }
                    Ok(true)
                }
                k => match line_input.input(k) {
                    None => Ok(false),
                    _ => Ok(true),
                },
            };
        }
    }

    let found_keymap = match config::get_config()
        .keymap_config
        .find_command_or_action_from_key_sequence(key_sequence)
    {
        Some(found) => found,
        None => return Ok(false),
    };

    let data = state.data.read();
    let search_results = data.caches.search.get(current_query);

    match focus_state {
        SearchFocusState::Input => anyhow::bail!("user's search input should be handled before"),
        SearchFocusState::Tracks => {
            let tracks = match search_results {
                Some(s) => s.tracks.iter().collect(),
                None => Vec::new(),
            };

            match found_keymap {
                CommandOrAction::Command(command) => window::handle_command_for_track_list_window(
                    command, client_pub, tracks, &data, ui,
                ),
                CommandOrAction::Action(action) => {
                    window::handle_action_for_selected_item(action, tracks, &data, ui, client_pub)
                }
            }
        }
        SearchFocusState::Artists => {
            let artists = search_results
                .map(|s| s.artists.iter().collect())
                .unwrap_or_default();

            match found_keymap {
                CommandOrAction::Command(command) => {
                    window::handle_command_for_artist_list_window(command, artists, &data, ui)
                }
                CommandOrAction::Action(action) => {
                    window::handle_action_for_selected_item(action, artists, &data, ui, client_pub)
                }
            }
        }
        SearchFocusState::Albums => {
            let albums = search_results
                .map(|s| s.albums.iter().collect())
                .unwrap_or_default();

            match found_keymap {
                CommandOrAction::Command(command) => window::handle_command_for_album_list_window(
                    command, albums, &data, ui, client_pub,
                ),
                CommandOrAction::Action(action) => {
                    window::handle_action_for_selected_item(action, albums, &data, ui, client_pub)
                }
            }
        }
        SearchFocusState::Playlists => {
            let playlists = search_results
                .map(|s| s.playlists.iter().collect())
                .unwrap_or_default();

            match found_keymap {
                CommandOrAction::Command(command) => {
                    window::handle_command_for_playlist_list_window(command, playlists, &data, ui)
                }
                CommandOrAction::Action(action) => window::handle_action_for_selected_item(
                    action, playlists, &data, ui, client_pub,
                ),
            }
        }
    }
}

fn handle_command_for_context_page(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    match command {
        Command::Search => {
            ui.new_search_popup();
            Ok(true)
        }
        _ => window::handle_command_for_focused_context_window(command, client_pub, ui, state),
    }
}

fn handle_action_for_browse_page(
    action: Action,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let data = state.data.read();

    match ui.current_page() {
        PageState::Browse { state } => match state {
            BrowsePageUIState::CategoryPlaylistList { category, .. } => {
                let playlists = match data.browse.category_playlists.get(&category.id) {
                    Some(v) => v,
                    None => return Ok(false),
                };

                let page_state = ui.current_page_mut();
                let selected = page_state.selected().unwrap_or_default();
                if selected >= playlists.len() {
                    return Ok(false);
                }

                handle_action_in_context(
                    action,
                    playlists[selected].clone().into(),
                    client_pub,
                    &data,
                    ui,
                )?;

                Ok(true)
            }
            _ => Ok(false),
        },
        _ => anyhow::bail!("expect a browse page state"),
    }
}

fn handle_command_for_browse_page(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
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

    if handle_navigation_command(command, page_state, selected, len) {
        return Ok(true);
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
                        ui.new_page(PageState::Browse {
                            state: BrowsePageUIState::CategoryPlaylistList {
                                category: categories[selected].clone(),
                                state: ListState::default(),
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
                        ui.new_page(PageState::Context {
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
            ui.new_search_popup();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(feature = "lyric-finder")]
fn handle_command_for_lyric_page(command: Command, ui: &mut UIStateGuard) -> Result<bool> {
    let scroll_offset = match ui.current_page() {
        PageState::Lyric { scroll_offset, .. } => *scroll_offset,
        _ => return Ok(false),
    };
    Ok(handle_navigation_command(
        command,
        ui.current_page_mut(),
        scroll_offset,
        10000,
    ))
}

fn handle_command_for_queue_page(
    command: Command,
    ui: &mut UIStateGuard,
) -> Result<bool, anyhow::Error> {
    let scroll_offset = match ui.current_page() {
        PageState::Queue { scroll_offset } => *scroll_offset,
        _ => return Ok(false),
    };
    Ok(handle_navigation_command(
        command,
        ui.current_page_mut(),
        scroll_offset,
        10000,
    ))
}

fn handle_command_for_command_help_page(command: Command, ui: &mut UIStateGuard) -> Result<bool> {
    let scroll_offset = match ui.current_page() {
        PageState::CommandHelp { scroll_offset } => *scroll_offset,
        _ => return Ok(false),
    };
    if command == Command::Search {
        ui.new_search_popup();
        return Ok(true);
    }
    Ok(handle_navigation_command(
        command,
        ui.current_page_mut(),
        scroll_offset,
        10000,
    ))
}

pub fn handle_navigation_command(
    command: Command,
    page: &mut PageState,
    id: usize,
    len: usize,
) -> bool {
    if len == 0 {
        return false;
    }

    let configs = config::get_config();
    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < len {
                page.select(id + 1);
            }
            true
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                page.select(id - 1);
            }
            true
        }
        Command::PageSelectNextOrScrollDown => {
            page.select(std::cmp::min(
                id + configs.app_config.page_size_in_rows,
                len - 1,
            ));
            true
        }
        Command::PageSelectPreviousOrScrollUp => {
            page.select(id.saturating_sub(configs.app_config.page_size_in_rows));
            true
        }
        Command::SelectLastOrScrollToBottom => {
            page.select(len - 1);
            true
        }
        Command::SelectFirstOrScrollToTop => {
            page.select(0);
            true
        }
        _ => false,
    }
}
