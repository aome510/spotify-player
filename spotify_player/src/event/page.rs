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
