use super::{construct_list_widget, help, Frame};
use crate::state::*;
use tui::{layout::*, widgets::*};

/// renders a popup (if any) to handle a command or show additional information
/// depending on the current popup state.
///
/// The function returns a rectangle area to render the main layout
/// and a boolean `is_active` determining whether the focus is **not** placed on the popup.
pub fn render_popup(frame: &mut Frame, state: &SharedState, rect: Rect) -> (Rect, bool) {
    let ui = state.ui.lock();

    match ui.popup {
        None => (rect, true),
        Some(ref popup) => match popup {
            PopupState::Search { query } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(rect);

                let widget = Paragraph::new(format!("/{}", query)).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(ui.theme.block_title_with_style("Search")),
                );
                frame.render_widget(widget, chunks[1]);
                (chunks[0], true)
            }
            PopupState::CommandHelp { .. } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                    .split(rect);

                drop(ui);
                help::render_commands_help_popup(frame, state, chunks[1]);
                (chunks[0], false)
            }
            PopupState::ActionList(item, _) => {
                let items = item
                    .actions()
                    .iter()
                    .map(|a| (format!("{:?}", a), false))
                    .collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "Actions", items, 7);
                (rect, false)
            }
            PopupState::DeviceList { .. } => {
                let player = state.player.read();

                let current_device_id = match player.playback {
                    Some(ref playback) => playback.device.id.as_deref().unwrap_or_default(),
                    None => "",
                };
                let items = player
                    .devices
                    .iter()
                    .map(|d| (format!("{} | {}", d.name, d.id), current_device_id == d.id))
                    .collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "Devices", items, 5);
                (rect, false)
            }
            PopupState::ThemeList(themes, ..) => {
                let items = themes.iter().map(|t| (t.name.clone(), false)).collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "Themes", items, 7);
                (rect, false)
            }
            PopupState::UserPlaylistList(action, _) => {
                let data = state.data.read();
                let playlists = match action {
                    PlaylistPopupAction::Browse => data.user_data.playlists.iter().collect(),
                    PlaylistPopupAction::AddTrack(_) => data.user_data.playlists_created_by_user(),
                };
                let items = playlists
                    .into_iter()
                    .map(|p| (p.name.clone(), false))
                    .collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "User Playlists", items, 10);
                (rect, false)
            }
            PopupState::UserFollowedArtistList { .. } => {
                let items = state
                    .data
                    .read()
                    .user_data
                    .followed_artists
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "User Followed Artists", items, 7);
                (rect, false)
            }
            PopupState::UserSavedAlbumList { .. } => {
                let items = state
                    .data
                    .read()
                    .user_data
                    .saved_albums
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "User Saved Albums", items, 7);
                (rect, false)
            }
            PopupState::ArtistList(artists, ..) => {
                let items = artists.iter().map(|a| (a.name.clone(), false)).collect();

                drop(ui);
                let rect = render_list_popup(frame, state, rect, "Artists", items, 5);
                (rect, false)
            }
        },
    }
}

/// a helper function to render a list popup
fn render_list_popup(
    frame: &mut Frame,
    state: &SharedState,
    rect: Rect,
    title: &'static str,
    items: Vec<(String, bool)>,
    length: u16,
) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(length)].as_ref())
        .split(rect);

    let widget = construct_list_widget(state, items, title, true, None);

    frame.render_stateful_widget(
        widget,
        chunks[1],
        state
            .ui
            .lock()
            .popup
            .as_mut()
            .unwrap()
            .list_state_mut()
            .unwrap(),
    );

    chunks[0]
}
