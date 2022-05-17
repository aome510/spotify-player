use super::*;
use crate::state::UIStateGuard;

/// Handles a command for the currently focused context window
///
/// The function will need to determine the focused window then
/// assign the handling job to such window's command handler
pub fn handle_command_for_focused_context_window(
    command: Command,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let ui = state.ui.lock();
    let context_uri = match ui.current_page() {
        PageState::Context { id, .. } => match id {
            None => return Ok(false),
            Some(id) => id.uri(),
        },
        _ => anyhow::bail!("expect a context page"),
    };

    match state.data.read().caches.context.peek(&context_uri) {
        Some(context) => match context {
            Context::Artist {
                top_tracks,
                albums,
                related_artists,
                ..
            } => {
                let focus_state = match ui.current_page() {
                    PageState::Context {
                        state: Some(ContextPageUIState::Artist { focus, .. }),
                        ..
                    } => focus,
                    _ => anyhow::bail!("expect an arist context page with a state"),
                };

                match focus_state {
                    ArtistFocusState::Albums => handle_command_for_album_list_window(
                        command,
                        ui.search_filtered_items(albums),
                        ui,
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list_window(
                        command,
                        ui.search_filtered_items(related_artists),
                        ui,
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table_window(
                        command,
                        client_pub,
                        None,
                        Some(top_tracks.iter().map(|t| &t.id).collect()),
                        ui.search_filtered_items(top_tracks),
                        ui,
                    ),
                }
            }
            Context::Album { album, tracks } => handle_command_for_track_table_window(
                command,
                client_pub,
                Some(ContextId::Album(album.id.clone())),
                None,
                ui.search_filtered_items(tracks),
                ui,
            ),
            Context::Playlist { playlist, tracks } => handle_command_for_track_table_window(
                command,
                client_pub,
                Some(ContextId::Playlist(playlist.id.clone())),
                None,
                ui.search_filtered_items(tracks),
                ui,
            ),
        },
        None => Ok(false),
    }
}

/// handles a command for the track table subwindow
///
/// In addition to the command and the application's states,
/// the function requires
/// - `tracks`: a list of tracks in the track table (can already be filtered by a search query)
/// - **either** `track_ids` or `context_id`
///
/// If `track_ids` is specified, playing a track in the track table will
/// start a `URIs` playback consisting of tracks whose id is in `track_ids`.
/// The above case is only used for the top-track table in an **Artist** context window.
///
/// If `context_id` is specified, playing a track in the track table will
/// start a `Context` playback representing a Spotify context.
/// The above case is used for the track table of a playlist or an album.
pub fn handle_command_for_track_table_window(
    command: Command,
    client_pub: &mpsc::Sender<ClientRequest>,
    context_id: Option<ContextId>,
    track_ids: Option<Vec<&TrackId>>,
    tracks: Vec<&Track>,
    mut ui: UIStateGuard,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < tracks.len() {
                ui.current_page_mut().select(id + 1);
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.current_page_mut().select(id - 1);
            }
        }
        Command::ChooseSelected => {
            let offset = Some(rspotify_model::Offset::for_uri(&tracks[id].id.uri()));
            if track_ids.is_some() {
                // play a track from a list of tracks
                client_pub.blocking_send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::URIs(track_ids.unwrap().into_iter().cloned().collect(), offset),
                )))?;
            } else if context_id.is_some() {
                // play a track from a context
                client_pub.blocking_send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::Context(context_id.unwrap(), offset),
                )))?;
            }
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Track(tracks[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn handle_command_for_track_list_window(
    command: Command,
    client_pub: &mpsc::Sender<ClientRequest>,
    tracks: Vec<&Track>,
    mut ui: UIStateGuard,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < tracks.len() {
                ui.current_page_mut().select(id + 1);
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.current_page_mut().select(id - 1);
            }
        }
        Command::ChooseSelected => {
            // for the track list, `ChooseSelected` on a track
            // will start a `URIs` playback containing only that track.
            // It's different for the track table, in which
            // `ChooseSelected` on a track will start a `URIs` playback
            // containing all the tracks in the table.
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(vec![tracks[id].id.clone()], None),
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Track(tracks[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn handle_command_for_artist_list_window(
    command: Command,
    artists: Vec<&Artist>,
    mut ui: UIStateGuard,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= artists.len() {
        return Ok(false);
    }

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < artists.len() {
                ui.current_page_mut().select(id + 1);
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.current_page_mut().select(id - 1);
            }
        }
        Command::ChooseSelected => {
            let context_id = ContextId::Artist(artists[id].id.clone());
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Artist(artists[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn handle_command_for_album_list_window(
    command: Command,
    albums: Vec<&Album>,
    mut ui: UIStateGuard,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= albums.len() {
        return Ok(false);
    }

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < albums.len() {
                ui.current_page_mut().select(id + 1);
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.current_page_mut().select(id - 1);
            }
        }
        Command::ChooseSelected => {
            let context_id = ContextId::Album(albums[id].id.clone());
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Album(albums[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn handle_command_for_playlist_list_window(
    command: Command,
    playlists: Vec<&Playlist>,
    mut ui: UIStateGuard,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= playlists.len() {
        return Ok(false);
    }

    match command {
        Command::SelectNextOrScrollDown => {
            if id + 1 < playlists.len() {
                ui.current_page_mut().select(id + 1);
            }
        }
        Command::SelectPreviousOrScrollUp => {
            if id > 0 {
                ui.current_page_mut().select(id - 1);
            }
        }
        Command::ChooseSelected => {
            let context_id = ContextId::Playlist(playlists[id].id.clone());
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            ui.popup = Some(PopupState::ActionList(
                Item::Playlist(playlists[id].clone()),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
