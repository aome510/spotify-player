use super::{construct_list_widget, help, Frame};
use crate::state::*;
use tui::{layout::*, widgets::*};

/// renders a popup (if any) to handle a command or show additional information
/// depending on the current popup state.
///
/// The function returns a rectangle area to render the main layout
/// and a boolean `is_active` determining whether the focus is **not** placed on the popup.
pub fn render_popup(frame: &mut Frame, state: &SharedState, rect: Rect) -> (Rect, bool) {
    let mut ui = state.ui.lock();

    match ui.popup {
        None => (rect, true),
        Some(ref popup) => match popup {
            PopupState::Search { query } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(rect);

                let widget = Paragraph::new(query.as_ref()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(ui.theme.block_title_with_style("Search")),
                );
                frame.render_widget(widget, rect);
                (chunks[0], true)
            }
            PopupState::CommandHelp { .. } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                    .split(rect);

                help::render_commands_help_popup(frame, &mut ui, state, chunks[1]);
                (chunks[0], false)
            }
            PopupState::ActionList(item, _) => {
                let items = item
                    .actions()
                    .iter()
                    .map(|a| (format!("{:?}", a), false))
                    .collect();

                let rect = render_list_popup(frame, &mut ui, rect, "Actions", items, 7);
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

                let rect = render_list_popup(frame, &mut ui, rect, "Devices", items, 5);
                (rect, false)
            }
            PopupState::ThemeList(themes, ..) => {
                let items = themes.iter().map(|t| (t.name.clone(), false)).collect();

                let rect = render_list_popup(frame, &mut ui, rect, "Themes", items, 7);
                (rect, false)
            }
            PopupState::UserPlaylistList(_, playlists, _) => {
                let items = playlists.iter().map(|p| (p.name.clone(), false)).collect();

                let rect = render_list_popup(frame, &mut ui, rect, "User Playlists", items, 10);
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

                let rect =
                    render_list_popup(frame, &mut ui, rect, "User Followed Artists", items, 7);
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

                let rect = render_list_popup(frame, &mut ui, rect, "User Saved Albums", items, 7);
                (rect, false)
            }
            PopupState::ArtistList(artists, ..) => {
                let items = artists.iter().map(|a| (a.name.clone(), false)).collect();

                let rect = render_list_popup(frame, &mut ui, rect, "Artists", items, 5);
                (rect, false)
            }
        },
    }
}

/// a helper function to render a list popup
fn render_list_popup(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    rect: Rect,
    title: &'static str,
    items: Vec<(String, bool)>,
    length: u16,
) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(length)].as_ref())
        .split(rect);

    let widget = construct_list_widget(ui, items, title, true, None);

    frame.render_stateful_widget(
        widget,
        chunks[1],
        ui.popup.as_mut().unwrap().list_state_mut().unwrap(),
    );

    chunks[0]
}
