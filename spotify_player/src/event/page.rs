use anyhow::Context as _;

use super::*;

macro_rules! handle_navigation_command {
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
pub(super) use handle_navigation_command;

pub fn handle_key_sequence_for_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let page_type = state.ui.lock().current_page().page_type();
    if page_type == PageType::Search {
        return handle_key_sequence_for_search_page(key_sequence, client_pub, state);
    }

    let command = match state
        .configs
        .keymap_config
        .find_command_from_key_sequence(key_sequence)
    {
        Some(command) => command,
        None => return Ok(false),
    };

    let ui = state.ui.lock();

    match page_type {
        PageType::Search => anyhow::bail!("page search type should already be handled!"),
        PageType::Library => handle_command_for_library_page(command, ui, state),
        PageType::Context => handle_command_for_context_page(command, client_pub, ui, state),
        PageType::Browse => handle_command_for_browse_page(command, client_pub, ui, state),
        #[cfg(feature = "lyric-finder")]
        PageType::Lyric => handle_command_for_lyric_page(command, ui, state),
        PageType::Queue => handle_command_for_queue_page(command, ui, state),
        PageType::CommandHelp => handle_command_for_command_help_page(command, ui, state),
    }
}

fn handle_command_for_library_page(
    command: Command,
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
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

fn handle_key_sequence_for_search_page(
    key_sequence: &KeySequence,
    client_pub: &flume::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

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

fn handle_command_for_context_page(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    match command {
        Command::Search => {
            ui.current_page_mut().select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
            Ok(true)
        }
        _ => window::handle_command_for_focused_context_window(command, client_pub, ui, state),
    }
}

fn handle_command_for_browse_page(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    mut ui: UIStateGuard,
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

    handle_navigation_command!(state, command, len, page_state, selected);
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
fn handle_command_for_lyric_page(
    command: Command,
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let scroll_offset = match ui.current_page_mut() {
        PageState::Lyric {
            ref mut scroll_offset,
            ..
        } => scroll_offset,
        _ => return Ok(false),
    };
    Ok(handle_scroll_command(command, state, scroll_offset))
}

fn handle_command_for_queue_page(
    command: Command,
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool, anyhow::Error> {
    let scroll_offset = match ui.current_page_mut() {
        PageState::Queue {
            ref mut scroll_offset,
        } => scroll_offset,
        _ => return Ok(false),
    };
    Ok(handle_scroll_command(command, state, scroll_offset))
}

fn handle_command_for_command_help_page(
    command: Command,
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let scroll_offset = match ui.current_page_mut() {
        PageState::CommandHelp {
            ref mut scroll_offset,
        } => scroll_offset,
        _ => return Ok(false),
    };
    Ok(handle_scroll_command(command, state, scroll_offset))
}

fn handle_scroll_command(command: Command, state: &SharedState, scroll_offset: &mut usize) -> bool {
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
        // Don't know the size of the page, use a "big" number as the `scroll_offset` should be adjusted
        // accordingly in the page rendering function.
        Command::SelectLastOrScrollToBottom => {
            *scroll_offset = 10000;
        }
        _ => return false,
    }
    true
}
