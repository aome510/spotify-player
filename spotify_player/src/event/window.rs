use super::*;
use crate::{
    command::{AlbumAction, ArtistAction, PlaylistAction, TrackAction},
    state::UIStateGuard,
};

/// Handles a command for the currently focused context window
///
/// The function will need to determine the focused window then
/// assign the handling job to such window's command handler
pub fn handle_command_for_focused_context_window(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
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

    let data = state.data.read();
    match data.caches.context.peek(&context_uri) {
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
                        &data,
                        ui,
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list_window(
                        command,
                        ui.search_filtered_items(related_artists),
                        &data,
                        ui,
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table_window(
                        command,
                        client_pub,
                        None,
                        Some(top_tracks.iter().map(|t| t.id.as_ref()).collect()),
                        ui.search_filtered_items(top_tracks),
                        &data,
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
                &data,
                ui,
            ),
            Context::Playlist { playlist, tracks } => handle_command_for_track_table_window(
                command,
                client_pub,
                Some(ContextId::Playlist(playlist.id.clone())),
                None,
                ui.search_filtered_items(tracks),
                &data,
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
    client_pub: &flume::Sender<ClientRequest>,
    context_id: Option<ContextId>,
    track_ids: Option<Vec<TrackId>>,
    tracks: Vec<&Track>,
    data: &DataReadGuard,
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
            let offset = Some(rspotify_model::Offset::Uri(tracks[id].id.uri()));
            if track_ids.is_some() {
                // play a track from a list of tracks
                client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::URIs(
                        track_ids
                            .unwrap()
                            .into_iter()
                            .map(|id| id.into_static())
                            .collect(),
                        offset,
                    ),
                )))?;
            } else if context_id.is_some() {
                // play a track from a context
                client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                    Playback::Context(context_id.unwrap(), offset),
                )))?;
            }
        }
        Command::ShowActionsOnSelectedItem => {
            let mut actions = command::construct_track_actions(tracks[id], data);
            if let Some(ContextId::Playlist(_)) = context_id {
                actions.push(TrackAction::DeleteFromCurrentPlaylist);
            }
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Track(tracks[id].clone(), actions),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn handle_command_for_track_list_window(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    tracks: Vec<&Track>,
    data: &DataReadGuard,
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
            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(vec![tracks[id].id.clone()], None),
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = command::construct_track_actions(tracks[id], data);
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Track(tracks[id].clone(), actions),
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
    data: &DataReadGuard,
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
            let mut actions = vec![ArtistAction::GoToArtistRadio];
            if data
                .user_data
                .followed_artists
                .iter()
                .any(|a| a.id == artists[id].id)
            {
                actions.push(ArtistAction::Unfollow);
            } else {
                actions.push(ArtistAction::Follow);
            }
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Artist(artists[id].clone(), actions),
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
    data: &DataReadGuard,
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
            let mut actions = vec![
                AlbumAction::GoToArtist,
                AlbumAction::GoToAlbumRadio,
                AlbumAction::GoToArtistRadio,
            ];
            if data
                .user_data
                .saved_albums
                .iter()
                .any(|a| a.id == albums[id].id)
            {
                actions.push(AlbumAction::DeleteFromLibrary);
            } else {
                actions.push(AlbumAction::AddToLibrary);
            }
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Album(albums[id].clone(), actions),
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
    data: &DataReadGuard,
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
            let mut actions = vec![PlaylistAction::GoToPlaylistRadio];
            if data
                .user_data
                .playlists
                .iter()
                .any(|a| a.id == playlists[id].id)
            {
                actions.push(PlaylistAction::DeleteFromLibrary);
            } else {
                actions.push(PlaylistAction::AddToLibrary);
            }
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Playlist(playlists[id].clone(), actions),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
