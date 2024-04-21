use super::page::handle_navigation_command;
use super::*;
use crate::{
    command::{
        construct_album_actions, construct_artist_actions, construct_playlist_actions, TrackAction,
    },
    state::UIStateGuard,
};
use rand::Rng;

/// Handle a command for the currently focused context window
///
/// The function will need to determine the focused window then
/// assign the handling job to the window's command handler
pub fn handle_command_for_focused_context_window(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let context_id = match ui.current_page() {
        PageState::Context { id, .. } => match id {
            None => return Ok(false),
            Some(id) => id,
        },
        _ => anyhow::bail!("expect a context page"),
    };

    // handle commands that require access to data's mutable state
    {
        let order = match command {
            Command::SortTrackByTitle => Some(TrackOrder::TrackName),
            Command::SortTrackByAlbum => Some(TrackOrder::Album),
            Command::SortTrackByArtists => Some(TrackOrder::Artists),
            Command::SortTrackByAddedDate => Some(TrackOrder::AddedAt),
            Command::SortTrackByDuration => Some(TrackOrder::Duration),
            _ => None,
        };

        // sort ordering commands
        if let Some(order) = order {
            let mut data = state.data.write();
            if let Some(tracks) = data.context_tracks(context_id) {
                tracks.sort_by(|x, y| order.compare(x, y));
            }
            return Ok(true);
        }
        // reverse ordering command
        if command == Command::ReverseTrackOrder {
            let mut data = state.data.write();
            if let Some(tracks) = data.context_tracks(context_id) {
                tracks.reverse();
            }
            return Ok(true);
        }
    }

    let data = state.data.read();

    match data.caches.context.get(&context_id.uri()) {
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
                        state,
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list_window(
                        command,
                        ui.search_filtered_items(related_artists),
                        &data,
                        ui,
                        state,
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table_window(
                        command,
                        client_pub,
                        Playback::URIs(
                            top_tracks.iter().map(|t| t.id.clone_static()).collect(),
                            None,
                        ),
                        ui.search_filtered_items(top_tracks),
                        &data,
                        ui,
                        state,
                    ),
                }
            }
            Context::Album { tracks, .. } => handle_command_for_track_table_window(
                command,
                client_pub,
                Playback::Context(context_id.clone(), None),
                ui.search_filtered_items(tracks),
                &data,
                ui,
                state,
            ),
            Context::Playlist { tracks, playlist } => handle_command_for_playlist_track_table(
                &playlist.id,
                command,
                client_pub,
                Playback::Context(context_id.clone(), None),
                ui.search_filtered_items(tracks),
                &data,
                ui,
                state,
            ),
            Context::Tracks { tracks, .. } => handle_command_for_track_table_window(
                command,
                client_pub,
                Playback::URIs(tracks.iter().map(|t| t.id.clone_static()).collect(), None),
                ui.search_filtered_items(tracks),
                &data,
                ui,
                state,
            ),
        },
        None => Ok(false),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_command_for_playlist_track_table(
    playlist_id: &PlaylistId<'static>,
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    base_playback: Playback,
    tracks: Vec<&Track>,
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    let modifiable = data
        .user_data
        .modifiable_playlists()
        .iter()
        .any(|p| p.id.eq(playlist_id));

    // handle commands that may modify a playlist
    if modifiable {
        match command {
            Command::MovePlaylistItemUp => {
                if id > 0 {
                    client_pub.send(ClientRequest::ReorderPlaylistItems {
                        playlist_id: playlist_id.clone_static(),
                        insert_index: id - 1,
                        range_start: id,
                        range_length: None,
                        snapshot_id: None,
                    })?;
                    ui.current_page_mut().select(id - 1);
                }
                return Ok(true);
            }
            Command::MovePlaylistItemDown => {
                if id + 1 < tracks.len() {
                    client_pub.send(ClientRequest::ReorderPlaylistItems {
                        playlist_id: playlist_id.clone_static(),
                        insert_index: id + 1,
                        range_start: id,
                        range_length: None,
                        snapshot_id: None,
                    })?;
                    ui.current_page_mut().select(id + 1);
                };
                return Ok(true);
            }
            Command::ShowActionsOnSelectedItem => {
                let mut actions = command::construct_track_actions(tracks[id], data);
                actions.push(TrackAction::DeleteFromCurrentPlaylist);
                ui.popup = Some(PopupState::ActionList(
                    ActionListItem::Track(tracks[id].clone(), actions),
                    new_list_state(),
                ));
                return Ok(true);
            }
            _ => {}
        }
    }

    handle_command_for_track_table_window(
        command,
        client_pub,
        base_playback,
        tracks,
        data,
        ui,
        state,
    )
}

fn handle_command_for_track_table_window(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    base_playback: Playback,
    tracks: Vec<&Track>,
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    handle_navigation_command!(state, command, tracks.len(), ui.current_page_mut(), id);
    match command {
        Command::PlayRandom => {
            let id = rand::thread_rng().gen_range(0..tracks.len());

            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                base_playback.uri_offset(
                    tracks[id].id.uri(),
                    state.configs.app_config.tracks_playback_limit,
                ),
                None,
            )))?;
        }
        Command::ChooseSelected => {
            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                base_playback.uri_offset(
                    tracks[id].id.uri(),
                    state.configs.app_config.tracks_playback_limit,
                ),
                None,
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = command::construct_track_actions(tracks[id], data);
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Track(tracks[id].clone(), actions),
                new_list_state(),
            ));
        }
        Command::AddSelectedItemToQueue => {
            client_pub.send(ClientRequest::AddTrackToQueue(tracks[id].id.clone()))?;
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
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    handle_navigation_command!(state, command, tracks.len(), ui.current_page_mut(), id);
    match command {
        Command::ChooseSelected => {
            // for the track list, `ChooseSelected` on a track
            // will start a `URIs` playback containing only that track.
            // It's different for the track table, in which
            // `ChooseSelected` on a track will start a `URIs` playback
            // containing all the tracks in the table.
            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(vec![tracks[id].id.clone()], None),
                None,
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = command::construct_track_actions(tracks[id], data);
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Track(tracks[id].clone(), actions),
                new_list_state(),
            ));
        }
        Command::AddSelectedItemToQueue => {
            client_pub.send(ClientRequest::AddTrackToQueue(tracks[id].id.clone()))?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn handle_command_for_artist_list_window(
    command: Command,
    artists: Vec<&Artist>,
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= artists.len() {
        return Ok(false);
    }

    handle_navigation_command!(state, command, artists.len(), ui.current_page_mut(), id);
    match command {
        Command::ChooseSelected => {
            let context_id = ContextId::Artist(artists[id].id.clone());
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = construct_artist_actions(artists[id], data);
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
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= albums.len() {
        return Ok(false);
    }

    handle_navigation_command!(state, command, albums.len(), ui.current_page_mut(), id);
    match command {
        Command::ChooseSelected => {
            let context_id = ContextId::Album(albums[id].id.clone());
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = construct_album_actions(albums[id], data);
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
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= playlists.len() {
        return Ok(false);
    }

    handle_navigation_command!(state, command, playlists.len(), ui.current_page_mut(), id);
    match command {
        Command::ChooseSelected => {
            let context_id = ContextId::Playlist(playlists[id].id.clone());
            ui.create_new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = construct_playlist_actions(playlists[id], data);
            ui.popup = Some(PopupState::ActionList(
                ActionListItem::Playlist(playlists[id].clone(), actions),
                new_list_state(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
