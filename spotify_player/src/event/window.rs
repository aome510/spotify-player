use super::page::handle_navigation_commands_for_page;
use super::*;
use crate::{
    command::{AlbumAction, ArtistAction, PlaylistAction, TrackAction},
    state::UIStateGuard,
};
use rand::Rng;

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
            if let Some(tracks) = data.get_tracks_by_id_mut(context_id) {
                tracks.sort_by(|x, y| order.compare(x, y));
            }
            return Ok(true);
        }
        // reverse ordering command
        if command == Command::ReverseTrackOrder {
            let mut data = state.data.write();
            if let Some(tracks) = data.get_tracks_by_id_mut(context_id) {
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
                artist,
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
                        ContextId::Tracks(TracksId::new(
                            format!("artist-{}-top-tracks", artist.name),
                            "Artist Top Tracks",
                        )),
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
                context_id.clone(),
                Playback::Context(context_id.clone(), None),
                ui.search_filtered_items(tracks),
                &data,
                ui,
                state,
            ),
            Context::Playlist { tracks, .. } => handle_command_for_track_table_window(
                command,
                client_pub,
                context_id.clone(),
                Playback::Context(context_id.clone(), None),
                ui.search_filtered_items(tracks),
                &data,
                ui,
                state,
            ),
            Context::Tracks { tracks, .. } => handle_command_for_track_table_window(
                command,
                client_pub,
                context_id.clone(),
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
/// handles a command for the track table subwindow
pub fn handle_command_for_track_table_window(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    context_id: ContextId,
    base_playback: Playback,
    tracks: Vec<&Track>,
    data: &DataReadGuard,
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    handle_navigation_commands_for_page!(state, command, tracks.len(), ui.current_page_mut(), id);
    match command {
        Command::PlayRandom => {
            let id = rand::thread_rng().gen_range(0..tracks.len());

            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                base_playback
                    .uri_offset(tracks[id].id.uri(), state.app_config.tracks_playback_limit),
            )))?;
        }
        Command::ChooseSelected => {
            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                base_playback
                    .uri_offset(tracks[id].id.uri(), state.app_config.tracks_playback_limit),
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let mut actions = command::construct_track_actions(tracks[id], data);
            if let ContextId::Playlist(playlist_id) = context_id {
                actions.push(TrackAction::DeleteFromCurrentPlaylist);

                if let (Some(Context::Playlist { tracks, playlist }), Some(user_id)) = (
                    state.data.read().caches.context.get(&playlist_id.uri()),
                    state
                        .data
                        .read()
                        .user_data
                        .user
                        .as_ref()
                        .map(|user| &user.id),
                ) {
                    actions.append(&mut track_actions_for_playlist_owner(
                        tracks, id, user_id, playlist,
                    ));
                }
            }
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
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    handle_navigation_commands_for_page!(state, command, tracks.len(), ui.current_page_mut(), id);
    match command {
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
    mut ui: UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= artists.len() {
        return Ok(false);
    }

    handle_navigation_commands_for_page!(state, command, artists.len(), ui.current_page_mut(), id);
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
            let mut actions = vec![ArtistAction::GoToArtistRadio, ArtistAction::CopyArtistLink];
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
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= albums.len() {
        return Ok(false);
    }

    handle_navigation_commands_for_page!(state, command, albums.len(), ui.current_page_mut(), id);
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
            let mut actions = vec![
                AlbumAction::GoToArtist,
                AlbumAction::GoToAlbumRadio,
                AlbumAction::GoToArtistRadio,
                AlbumAction::CopyAlbumLink,
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
    state: &SharedState,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= playlists.len() {
        return Ok(false);
    }

    handle_navigation_commands_for_page!(
        state,
        command,
        playlists.len(),
        ui.current_page_mut(),
        id
    );
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
            let mut actions = vec![
                PlaylistAction::GoToPlaylistRadio,
                PlaylistAction::CopyPlaylistLink,
            ];
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

fn track_actions_for_playlist_owner(
    tracks: &[Track],
    track_index: usize,
    user_id: &UserId<'_>,
    playlist: &Playlist,
) -> Vec<TrackAction> {
    if &playlist.owner.1 != user_id {
        return vec![];
    };

    let mut actions = vec![];
    if track_index > 0 {
        actions.push(TrackAction::MoveUpInCurrentPlaylist);
    }
    if track_index + 1 < tracks.len() {
        actions.push(TrackAction::MoveDownInCurrentPlaylist);
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_actions_for_playlist_owner() {
        fn create_dummy_track(track_id: &str) -> Track {
            Track {
                id: TrackId::from_id(track_id.to_owned()).unwrap(),
                name: "".to_string(),
                artists: vec![],
                album: None,
                duration: chrono::Duration::minutes(1),
                added_at: 0,
            }
        }

        let user_id = "jhgDSLJahsgd";
        let tracks = vec![
            create_dummy_track("37BTh5g05cxBIRYMbw8g2T"),
            create_dummy_track("4cOdK2wGLETKBW3PvgPWqT"),
            create_dummy_track("6tASfEUyB7lE2r6DLzURji"),
        ];

        let playlist = Playlist {
            id: PlaylistId::from_id(user_id.to_owned()).unwrap(),
            collaborative: false,
            name: "".to_string(),
            owner: (
                user_id.to_string(),
                UserId::from_id(user_id.to_owned()).unwrap(),
            ),
        };

        // test when the track index is neither first or last
        let actions = track_actions_for_playlist_owner(
            &tracks,
            1,
            &UserId::from_id(user_id).unwrap(),
            &playlist,
        );
        assert!(actions.contains(&TrackAction::MoveUpInCurrentPlaylist));
        assert!(actions.contains(&TrackAction::MoveDownInCurrentPlaylist));

        // test when the track index is first
        let actions = track_actions_for_playlist_owner(
            &tracks,
            0,
            &UserId::from_id(user_id).unwrap(),
            &playlist,
        );
        assert!(!actions.contains(&TrackAction::MoveUpInCurrentPlaylist));
        assert!(actions.contains(&TrackAction::MoveDownInCurrentPlaylist));

        // test when the track index is last
        let actions = track_actions_for_playlist_owner(
            &tracks,
            2,
            &UserId::from_id(user_id).unwrap(),
            &playlist,
        );
        assert!(actions.contains(&TrackAction::MoveUpInCurrentPlaylist));
        assert!(!actions.contains(&TrackAction::MoveDownInCurrentPlaylist));
    }
}
