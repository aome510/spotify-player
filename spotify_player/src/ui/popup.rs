use super::{help, Frame};
use crate::state::*;
use tui::{layout::*, style::*, widgets::*};

pub fn render_popup(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) -> (Rect, bool) {
    // handle popup windows
    match ui.popup {
        PopupState::None => (rect, true),
        PopupState::CommandHelp => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                .split(rect);
            help::render_commands_help_widget(frame, ui, state, chunks[1]);
            (chunks[0], false)
        }
        PopupState::DeviceList(_) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(5)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let player = state.player.read().unwrap();
                    let current_device_id = match player.playback {
                        Some(ref playback) => &playback.device.id,
                        None => "",
                    };
                    let items = player
                        .devices
                        .iter()
                        .map(|d| (format!("{} | {}", d.name, d.id), current_device_id == d.id))
                        .collect();
                    construct_list_widget(ui, items, "Devices")
                },
                chunks[1],
                ui.popup.get_list_state_mut().unwrap(),
            );
            (chunks[0], false)
        }
        PopupState::ThemeList(ref themes, _) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let items = themes.iter().map(|t| (t.name.clone(), false)).collect();
                    construct_list_widget(ui, items, "Themes")
                },
                chunks[1],
                ui.popup.get_list_state_mut().unwrap(),
            );
            (chunks[0], false)
        }
        // TODO: reduce the code duplication between handlers for `UserPlaylists`, `UserSavedAlbums`, and `UserFollowedArtists`
        PopupState::UserPlaylistList(_) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let player = state.player.read().unwrap();
                    let items = player
                        .user_playlists
                        .as_ref()
                        .map(|playlists| {
                            playlists.iter().map(|p| (p.name.clone(), false)).collect()
                        })
                        .unwrap_or_default();
                    construct_list_widget(ui, items, "User Playlists")
                },
                chunks[1],
                ui.popup.get_list_state_mut().unwrap(),
            );
            (chunks[0], false)
        }
        PopupState::UserFollowedArtistList(_) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let player = state.player.read().unwrap();
                    let items = player
                        .user_followed_artists
                        .as_ref()
                        .map(|artists| artists.iter().map(|a| (a.name.clone(), false)).collect())
                        .unwrap_or_default();
                    construct_list_widget(ui, items, "User Followed Artists")
                },
                chunks[1],
                ui.popup.get_list_state_mut().unwrap(),
            );
            (chunks[0], false)
        }
        PopupState::UserSavedAlbumList(_) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let player = state.player.read().unwrap();
                    let items = player
                        .user_saved_albums
                        .as_ref()
                        .map(|albums| albums.iter().map(|a| (a.name.clone(), false)).collect())
                        .unwrap_or_default();
                    construct_list_widget(ui, items, "User Saved Albums")
                },
                chunks[1],
                ui.popup.get_list_state_mut().unwrap(),
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
                    construct_list_widget(ui, items, "Artists")
                },
                chunks[1],
                ui.popup.get_list_state_mut().unwrap(),
            );
            (chunks[0], false)
        }
        PopupState::ContextSearch(ref query) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                .split(rect);
            render_search_box_widget(frame, ui, chunks[1], format!("/{}", query));
            (chunks[0], true)
        }
    }
}

fn render_search_box_widget(frame: &mut Frame, ui: &UIStateGuard, rect: Rect, query: String) {
    let search_box = Paragraph::new(query).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.theme.block_title_with_style("Search")),
    );
    frame.render_widget(search_box, rect);
}

fn construct_list_widget<'a>(
    ui: &UIStateGuard,
    items: Vec<(String, bool)>,
    title: &str,
) -> List<'a> {
    List::new(
        items
            .into_iter()
            .map(|(s, is_active)| {
                ListItem::new(s).style(if is_active {
                    ui.theme.current_active()
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>(),
    )
    .highlight_style(ui.theme.selection_style(true))
    .block(
        Block::default()
            .title(ui.theme.block_title_with_style(title))
            .borders(Borders::ALL),
    )
}
