use super::{construct_list_widget, help, Frame};
use crate::state::*;
use tui::{layout::*, widgets::*};

/// renders a popup to handle a command or show additional information
/// depending on the current popup state
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
            PopupState::ActionList(..) => {
                todo!()
            }
            PopupState::CommandHelp(_) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                    .split(rect);

                help::render_commands_help_window(frame, ui, state, chunks[1]);
                (chunks[0], false)
            }
            PopupState::DeviceList(_) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(5)].as_ref())
                    .split(rect);

                let widget = {
                    let current_device_id = match player.playback {
                        Some(ref playback) => &playback.device.id,
                        None => "",
                    };
                    let items = player
                        .devices
                        .iter()
                        .map(|d| (format!("{} | {}", d.name, d.id), current_device_id == d.id))
                        .collect();
                    construct_list_widget(ui, items, "Devices", true, None)
                };

                frame.render_stateful_widget(
                    widget,
                    chunks[1],
                    ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
                );
                (chunks[0], false)
            }
            PopupState::ThemeList(ref themes, _) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
                    .split(rect);

                let widget = {
                    let items = themes.iter().map(|t| (t.name.clone(), false)).collect();
                    construct_list_widget(ui, items, "Themes", true, None)
                };

                frame.render_stateful_widget(
                    widget,
                    chunks[1],
                    ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
                );
                (chunks[0], false)
            }
            PopupState::UserPlaylistList(_) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                    .split(rect);

                let widget = {
                    let items = player
                        .user_playlists
                        .iter()
                        .map(|p| (p.name.clone(), false))
                        .collect();
                    construct_list_widget(ui, items, "User Playlists", true, None)
                };

                frame.render_stateful_widget(
                    widget,
                    chunks[1],
                    ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
                );
                (chunks[0], false)
            }
            PopupState::UserFollowedArtistList(_) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                    .split(rect);

                let widget = {
                    let items = player
                        .user_followed_artists
                        .iter()
                        .map(|a| (a.name.clone(), false))
                        .collect();
                    construct_list_widget(ui, items, "User Followed Artists", true, None)
                };

                frame.render_stateful_widget(
                    widget,
                    chunks[1],
                    ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
                );
                (chunks[0], false)
            }
            PopupState::UserSavedAlbumList(_) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                    .split(rect);

                let widget = {
                    let items = player
                        .user_saved_albums
                        .iter()
                        .map(|a| (a.name.clone(), false))
                        .collect();
                    construct_list_widget(ui, items, "User Saved Albums", true, None)
                };

                frame.render_stateful_widget(
                    widget,
                    chunks[1],
                    ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
                );
                (chunks[0], false)
            }
            PopupState::ArtistList(ref artists, _) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(5)].as_ref())
                    .split(rect);

                frame.render_stateful_widget(
                    {
                        let items = artists.iter().map(|a| (a.name.clone(), false)).collect();
                        construct_list_widget(ui, items, "Artists", true, None)
                    },
                    chunks[1],
                    ui.popup.as_mut().unwrap().get_list_state_mut().unwrap(),
                );
                (chunks[0], false)
            }
            PopupState::ContextSearch(ref query) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(rect);

                render_context_search_box(frame, ui, chunks[1], format!("/{}", query));
                (chunks[0], true)
            }
        },
    }
}

fn render_context_search_box(frame: &mut Frame, ui: &UIStateGuard, rect: Rect, query: String) {
    let search_box = Paragraph::new(query).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.theme.block_title_with_style("Search")),
    );
    frame.render_widget(search_box, rect);
}
