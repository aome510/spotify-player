use super::{construct_list_widget, help, Frame};
use crate::state::*;
use tui::{layout::*, widgets::*};

/// renders a popup (if any) to handle a command or show additional information
/// depending on the current popup state.
///
/// The function returns the remaining rectangle area to draw the main layout
/// and a boolean `is_active` to determine whether the focus is **not** placed on the popup.
pub fn render_popup(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) -> (Rect, bool) {
    let player = state.player.read().unwrap();

    match ui.popup {
        None => (rect, true),
        Some(ref popup) => match popup {
            PopupState::ContextSearch(ref query) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(rect);

                render_context_search_box(frame, ui, chunks[1], format!("/{}", query));
                (chunks[0], true)
            }
            PopupState::CommandHelp(_) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                    .split(rect);

                help::render_commands_help_window(frame, ui, state, chunks[1]);
                (chunks[0], false)
            }
            PopupState::ActionList(item, _) => {
                let items = item
                    .actions()
                    .into_iter()
                    .map(|a| (format!("{:?}", a), false))
                    .collect();

                let rect = render_list_popup(frame, ui, rect, "User Saved Albums", items, 7);
                (rect, false)
            }
            PopupState::DeviceList(_) => {
                let current_device_id = match player.playback {
                    Some(ref playback) => &playback.device.id,
                    None => "",
                };
                let items = player
                    .devices
                    .iter()
                    .map(|d| (format!("{} | {}", d.name, d.id), current_device_id == d.id))
                    .collect();

                let rect = render_list_popup(frame, ui, rect, "Devices", items, 5);
                (rect, false)
            }
            PopupState::ThemeList(ref themes, _) => {
                let items = themes.iter().map(|t| (t.name.clone(), false)).collect();

                let rect = render_list_popup(frame, ui, rect, "Themes", items, 7);
                (rect, false)
            }
            PopupState::UserPlaylistList(_) => {
                let items = player
                    .user_playlists
                    .iter()
                    .map(|p| (p.name.clone(), false))
                    .collect();

                let rect = render_list_popup(frame, ui, rect, "User Playlists", items, 10);
                (rect, false)
            }
            PopupState::UserFollowedArtistList(_) => {
                let items = player
                    .user_followed_artists
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect();

                let rect = render_list_popup(frame, ui, rect, "User Followed Artists", items, 7);
                (rect, false)
            }
            PopupState::UserSavedAlbumList(_) => {
                let items = player
                    .user_saved_albums
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect();

                let rect = render_list_popup(frame, ui, rect, "User Saved Albums", items, 7);
                (rect, false)
            }
            PopupState::ArtistList(ref artists, _) => {
                let items = artists.iter().map(|a| (a.name.clone(), false)).collect();

                let rect = render_list_popup(frame, ui, rect, "Artists", items, 5);
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
        ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
    );

    chunks[0]
}

fn render_context_search_box(frame: &mut Frame, ui: &UIStateGuard, rect: Rect, query: String) {
    let search_box = Paragraph::new(query).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.theme.block_title_with_style("Search")),
    );
    frame.render_widget(search_box, rect);
}
