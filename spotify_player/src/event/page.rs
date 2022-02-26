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

    match command {
        Command::Search => {
            let mut ui = state.ui.lock();
            ui.current_page_mut().select(0);
            ui.popup = Some(PopupState::Search {
                query: "".to_owned(),
            });
            Ok(true)
        }
        _ => {
            let ui = state.ui.lock();
            let data = state.data.read();
            let focus_state = match ui.current_page() {
                PageState::Library { state } => state.focus,
                _ => unreachable!("expect a library page state"),
            };
            match focus_state {
                LibraryFocusState::Playlists => {
                    let items = ui.search_filtered_items(&data.user_data.playlists);
                    window::handle_command_for_playlist_list_window(command, ui, items)
                }
                LibraryFocusState::SavedAlbums => {
                    let items = ui.search_filtered_items(&data.user_data.saved_albums);
                    window::handle_command_for_album_list_window(command, ui, items)
                }
                LibraryFocusState::FollowedArtists => {
                    let items = ui.search_filtered_items(&data.user_data.followed_artists);
                    window::handle_command_for_artist_list_window(command, ui, items)
                }
            }
        }
    }
}

pub fn handle_key_sequence_for_search_page(
    key_sequence: &KeySequence,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

    let (focus_state, input, current_query) = match ui.current_page_mut() {
        PageState::Search {
            state,
            input,
            current_query,
        } => (state.focus, input, current_query),
        _ => unreachable!("expect a search page"),
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
                            client_pub.blocking_send(ClientRequest::Search(input.clone()))?;
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
        SearchFocusState::Input => unreachable!("user's search input should be handled before"),
        SearchFocusState::Tracks => {
            let tracks = search_results
                .map(|s| s.tracks.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_track_list_window(command, client_pub, ui, tracks)
        }
        SearchFocusState::Artists => {
            let artists = search_results
                .map(|s| s.artists.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_artist_list_window(command, ui, artists)
        }
        SearchFocusState::Albums => {
            let albums = search_results
                .map(|s| s.albums.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_album_list_window(command, ui, albums)
        }
        SearchFocusState::Playlists => {
            let playlists = search_results
                .map(|s| s.playlists.iter().collect())
                .unwrap_or_default();
            window::handle_command_for_playlist_list_window(command, ui, playlists)
        }
    }
}
