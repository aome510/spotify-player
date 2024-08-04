use super::page::handle_navigation_command;
use super::*;
use crate::{
    command::{construct_album_actions, construct_artist_actions, construct_playlist_actions},
    state::UIStateGuard,
};
use command::Action;
use rand::Rng;

pub fn handle_action_for_focused_context_page(
    action: Action,
    client_pub: &flume::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
    state: &SharedState,
) -> Result<bool> {
    let context_id = match ui.current_page() {
        PageState::Context { id: Some(id), .. } => id,
        _ => return Ok(false),
    };

    let data = state.data.read();
    match data.caches.context.get(&context_id.uri()) {
        Some(Context::Artist {
            top_tracks,
            albums,
            related_artists,
            ..
        }) => {
            let focus_state = match ui.current_page() {
                PageState::Context {
                    state: Some(ContextPageUIState::Artist { focus, .. }),
                    ..
                } => focus,
                _ => return Ok(false),
            };

            match focus_state {
                ArtistFocusState::Albums => handle_action_for_selected_item(
                    action,
                    ui.search_filtered_items(albums),
                    &data,
                    ui,
                    client_pub,
                ),
                ArtistFocusState::RelatedArtists => handle_action_for_selected_item(
                    action,
                    ui.search_filtered_items(related_artists),
                    &data,
                    ui,
                    client_pub,
                ),
                ArtistFocusState::TopTracks => handle_action_for_selected_item(
                    action,
                    ui.search_filtered_items(top_tracks),
                    &data,
                    ui,
                    client_pub,
                ),
            }
        }
        Some(Context::Album { tracks, .. }) => handle_action_for_selected_item(
            action,
            ui.search_filtered_items(tracks),
            &data,
            ui,
            client_pub,
        ),
        Some(Context::Tracks { tracks, .. }) => handle_action_for_selected_item(
            action,
            ui.search_filtered_items(tracks),
            &data,
            ui,
            client_pub,
        ),
        Some(Context::Playlist { tracks, .. }) => handle_action_for_selected_item(
            action,
            ui.search_filtered_items(tracks),
            &data,
            ui,
            client_pub,
        ),
        None => Ok(false),
    }
}

pub fn handle_action_for_selected_item<T: Into<ActionContext> + Clone>(
    action: Action,
    items: Vec<&T>,
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
    client_pub: &flume::Sender<ClientRequest>,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= items.len() {
        return Ok(false);
    }

    handle_action_in_context(action, items[id].clone().into(), client_pub, data, ui)?;

    Ok(true)
}

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
            if let Some(tracks) = data.context_tracks_mut(context_id) {
                tracks.sort_by(|x, y| order.compare(x, y));
            }
            return Ok(true);
        }
        // reverse ordering command
        if command == Command::ReverseTrackOrder {
            let mut data = state.data.write();
            if let Some(tracks) = data.context_tracks_mut(context_id) {
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
                        client_pub,
                    ),
                    ArtistFocusState::RelatedArtists => handle_command_for_artist_list_window(
                        command,
                        ui.search_filtered_items(related_artists),
                        &data,
                        ui,
                    ),
                    ArtistFocusState::TopTracks => handle_command_for_track_table_window(
                        command, client_pub, None, top_tracks, &data, ui,
                    ),
                }
            }
            Context::Album { tracks, .. } => handle_command_for_track_table_window(
                command,
                client_pub,
                Some(context_id.clone()),
                tracks,
                &data,
                ui,
            ),
            Context::Playlist { tracks, .. } => handle_command_for_track_table_window(
                command,
                client_pub,
                Some(context_id.clone()),
                tracks,
                &data,
                ui,
            ),
            Context::Tracks { tracks, .. } => {
                handle_command_for_track_table_window(command, client_pub, None, tracks, &data, ui)
            }
        },
        None => Ok(false),
    }
}

/// Handle commands that may modify a playlist
fn handle_playlist_modify_command(
    id: usize,
    playlist_id: &PlaylistId<'static>,
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    tracks: &[&Track],
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
) -> Result<bool> {
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
            actions.push(Action::DeleteFromPlaylist);
            ui.popup = Some(PopupState::ActionList(
                Box::new(ActionListItem::Track(tracks[id].clone(), actions)),
                ListState::default(),
            ));
            return Ok(true);
        }
        _ => {}
    }

    Ok(false)
}

fn handle_command_for_track_table_window(
    command: Command,
    client_pub: &flume::Sender<ClientRequest>,
    context_id: Option<ContextId>,
    tracks: &[Track],
    data: &DataReadGuard,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    let filtered_tracks = ui.search_filtered_items(tracks);
    if id >= filtered_tracks.len() {
        return Ok(false);
    }

    if let Some(ContextId::Playlist(ref playlist_id)) = context_id {
        let modifiable = data
            .user_data
            .modifiable_playlists()
            .iter()
            .any(|p| p.id.eq(playlist_id));
        if modifiable
            && handle_playlist_modify_command(
                id,
                playlist_id,
                command,
                client_pub,
                &filtered_tracks,
                data,
                ui,
            )?
        {
            return Ok(true);
        }
    }

    if handle_navigation_command(command, ui.current_page_mut(), id, filtered_tracks.len()) {
        return Ok(true);
    }

    match command {
        Command::PlayRandom | Command::ChooseSelected => {
            let uri = if command == Command::PlayRandom {
                tracks[rand::thread_rng().gen_range(0..tracks.len())]
                    .id
                    .uri()
            } else {
                filtered_tracks[id].id.uri()
            };

            let base_playback = if let Some(context_id) = context_id {
                Playback::Context(context_id, None)
            } else {
                Playback::URIs(tracks.iter().map(|t| t.id.clone_static()).collect(), None)
            };

            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                base_playback
                    .uri_offset(uri, config::get_config().app_config.tracks_playback_limit),
                None,
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = command::construct_track_actions(filtered_tracks[id], data);
            ui.popup = Some(PopupState::ActionList(
                Box::new(ActionListItem::Track(tracks[id].clone(), actions)),
                ListState::default(),
            ));
        }
        Command::AddSelectedItemToQueue => {
            client_pub.send(ClientRequest::AddTrackToQueue(
                filtered_tracks[id].id.clone(),
            ))?;
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
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= tracks.len() {
        return Ok(false);
    }

    if handle_navigation_command(command, ui.current_page_mut(), id, tracks.len()) {
        return Ok(true);
    }
    match command {
        Command::ChooseSelected => {
            // for a track list, `ChooseSelected` on a track
            // will start a `URIs` playback containing only that track.
            // This is different from the track table, which handles
            // `ChooseSelected` by starting a `URIs` playback
            // containing all the tracks in the table.
            client_pub.send(ClientRequest::Player(PlayerRequest::StartPlayback(
                Playback::URIs(vec![tracks[id].id.clone()], None),
                None,
            )))?;
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = command::construct_track_actions(tracks[id], data);
            ui.popup = Some(PopupState::ActionList(
                Box::new(ActionListItem::Track(tracks[id].clone(), actions)),
                ListState::default(),
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
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= artists.len() {
        return Ok(false);
    }

    if handle_navigation_command(command, ui.current_page_mut(), id, artists.len()) {
        return Ok(true);
    }
    match command {
        Command::ChooseSelected => {
            let context_id = ContextId::Artist(artists[id].id.clone());
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = construct_artist_actions(artists[id], data);
            ui.popup = Some(PopupState::ActionList(
                Box::new(ActionListItem::Artist(artists[id].clone(), actions)),
                ListState::default(),
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
    client_pub: &flume::Sender<ClientRequest>,
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= albums.len() {
        return Ok(false);
    }

    if handle_navigation_command(command, ui.current_page_mut(), id, albums.len()) {
        return Ok(true);
    }
    match command {
        Command::ChooseSelected => {
            let context_id = ContextId::Album(albums[id].id.clone());
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = construct_album_actions(albums[id], data);
            ui.popup = Some(PopupState::ActionList(
                Box::new(ActionListItem::Album(albums[id].clone(), actions)),
                ListState::default(),
            ));
        }
        Command::AddSelectedItemToQueue => {
            client_pub.send(ClientRequest::AddAlbumToQueue(albums[id].id.clone()))?;
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
) -> Result<bool> {
    let id = ui.current_page_mut().selected().unwrap_or_default();
    if id >= playlists.len() {
        return Ok(false);
    }

    if handle_navigation_command(command, ui.current_page_mut(), id, playlists.len()) {
        return Ok(true);
    }
    match command {
        Command::ChooseSelected => {
            let context_id = ContextId::Playlist(playlists[id].id.clone());
            ui.new_page(PageState::Context {
                id: None,
                context_page_type: ContextPageType::Browsing(context_id),
                state: None,
            });
        }
        Command::ShowActionsOnSelectedItem => {
            let actions = construct_playlist_actions(playlists[id], data);
            ui.popup = Some(PopupState::ActionList(
                Box::new(ActionListItem::Playlist(playlists[id].clone(), actions)),
                ListState::default(),
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
