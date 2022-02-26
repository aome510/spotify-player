// use super::*;

// pub fn handle_key_sequence_for_library_page(
//     key_sequence: &KeySequence,
//     state: &SharedState,
//     page_ui_state: &LibraryPageUIState,
// ) -> Result<bool> {
//     let command = match state
//         .keymap_config
//         .find_command_from_key_sequence(key_sequence)
//     {
//         Some(command) => command,
//         None => return Ok(false),
//     };

//     match command {
//         Command::Search => {
//             let mut ui = state.ui.lock();
//             ui.window.select(Some(0));
//             ui.popup = Some(PopupState::Search {
//                 query: "".to_owned(),
//             });
//             Ok(true)
//         }
//         _ => {
//             let data = state.data.read();
//             match focus_state {
//                 LibraryFocusState::Playlists => handle_command_for_playlist_list_subwindow(
//                     command,
//                     state,
//                     state.filtered_items_by_search(&data.user_data.playlists),
//                 ),
//                 LibraryFocusState::SavedAlbums => handle_command_for_album_list_subwindow(
//                     command,
//                     state,
//                     state.filtered_items_by_search(&data.user_data.saved_albums),
//                 ),
//                 LibraryFocusState::FollowedArtists => handle_command_for_artist_list_subwindow(
//                     command,
//                     state,
//                     state.filtered_items_by_search(&data.user_data.followed_artists),
//                 ),
//             }
//         }
//     }
// }
