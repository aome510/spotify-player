use super::*;
use std::collections::{btree_map::Entry, BTreeMap};

const SHORTCUT_TABLE_N_COLUMNS: usize = 3;
const SHORTCUT_TABLE_CONSTRAINS: [Constraint; SHORTCUT_TABLE_N_COLUMNS] = [
    Constraint::Percentage(33),
    Constraint::Percentage(33),
    Constraint::Percentage(33),
];
const COMMAND_TABLE_CONSTRAINTS: [Constraint; 3] = [
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(50),
];

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

                render_commands_help_popup(frame, state, ui, chunks[1]);
                (chunks[0], false)
            }
            PopupState::ActionList(item, _) => {
                let items = item
                    .actions()
                    .iter()
                    .map(|a| (format!("{:?}", a), false))
                    .collect();

                let rect = render_list_popup(
                    frame,
                    rect,
                    &format!("Actions on {}", item.name()),
                    items,
                    7,
                    ui,
                );
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

                let rect = render_list_popup(frame, rect, "Devices", items, 5, ui);
                (rect, false)
            }
            PopupState::ThemeList(themes, ..) => {
                let items = themes.iter().map(|t| (t.name.clone(), false)).collect();

                let rect = render_list_popup(frame, rect, "Themes", items, 7, ui);
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

                let rect = render_list_popup(frame, rect, "User Playlists", items, 10, ui);
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

                let rect = render_list_popup(frame, rect, "User Followed Artists", items, 7, ui);
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

                let rect = render_list_popup(frame, rect, "User Saved Albums", items, 7, ui);
                (rect, false)
            }
            PopupState::ArtistList(artists, ..) => {
                let items = artists.iter().map(|a| (a.name.clone(), false)).collect();

                let rect = render_list_popup(frame, rect, "Artists", items, 5, ui);
                (rect, false)
            }
        },
    }
}

/// a helper function to render a list popup
fn render_list_popup(
    frame: &mut Frame,
    rect: Rect,
    title: &str,
    items: Vec<(String, bool)>,
    length: u16,
    mut ui: UIStateGuard,
) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(length)].as_ref())
        .split(rect);

    let widget = construct_list_widget(&ui.theme, items, title, true, None);

    frame.render_stateful_widget(
        widget,
        chunks[1],
        ui.popup.as_mut().unwrap().list_state_mut().unwrap(),
    );

    chunks[0]
}

/// renders a shortcut help popup to show the available shortcuts based on user's inputs
pub fn render_shortcut_help_popup(frame: &mut Frame, state: &SharedState, rect: Rect) -> Rect {
    let ui = state.ui.lock();
    let input = &ui.input_key_sequence;

    // get the matches (keymaps) from the current key sequence input,
    // if there is at lease one match, render the shortcut help popup
    let matches = {
        if input.keys.is_empty() {
            vec![]
        } else {
            state
                .keymap_config
                .find_matched_prefix_keymaps(input)
                .into_iter()
                .map(|keymap| {
                    let mut keymap = keymap.clone();
                    keymap.key_sequence.keys.drain(0..input.keys.len());
                    keymap
                })
                .filter(|keymap| !keymap.key_sequence.keys.is_empty())
                .collect::<Vec<_>>()
        }
    };

    if matches.is_empty() {
        rect
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
            .split(rect);

        let help_table = Table::new(
            matches
                .into_iter()
                .map(|km| format!("{}: {:?}", km.key_sequence, km.command))
                .collect::<Vec<_>>()
                .chunks(SHORTCUT_TABLE_N_COLUMNS)
                .map(|c| Row::new(c.iter().map(|i| Cell::from(i.to_owned()))))
                .collect::<Vec<_>>(),
        )
        .widths(&SHORTCUT_TABLE_CONSTRAINS)
        .block(
            Block::default()
                .title(ui.theme.block_title_with_style("Shortcuts"))
                .borders(Borders::ALL),
        );
        frame.render_widget(help_table, chunks[1]);
        chunks[0]
    }
}

/// renders a command help popup listing all key shortcuts and corresponding descriptions
pub fn render_commands_help_popup(
    frame: &mut Frame,
    state: &SharedState,
    mut ui: UIStateGuard,
    rect: Rect,
) {
    let offset = match ui.popup {
        Some(PopupState::CommandHelp { ref mut offset }) => offset,
        _ => return,
    };

    let mut map = BTreeMap::new();
    state.keymap_config.keymaps.iter().for_each(|km| {
        let v = map.entry(km.command);
        match v {
            Entry::Vacant(v) => {
                v.insert(format!("\"{}\"", km.key_sequence));
            }
            Entry::Occupied(mut v) => {
                let desc = format!("{}, \"{}\"", v.get(), km.key_sequence);
                *v.get_mut() = desc;
            }
        }
    });

    // offset should not be greater than or equal the number of available commands
    if *offset >= map.len() {
        *offset = map.len() - 1
    }
    let help_table = Table::new(
        map.into_iter()
            .skip(*offset)
            .map(|(c, k)| {
                Row::new(vec![
                    Cell::from(format!("{:?}", c)),
                    Cell::from(format!("[{}]", k)),
                    Cell::from(c.desc()),
                ])
            })
            .collect::<Vec<_>>(),
    )
    .header(
        Row::new(vec![
            Cell::from("Command"),
            Cell::from("Shortcuts"),
            Cell::from("Description"),
        ])
        .style(ui.theme.table_header()),
    )
    .widths(&COMMAND_TABLE_CONSTRAINTS)
    .block(
        Block::default()
            .title(ui.theme.block_title_with_style("Commands"))
            .borders(Borders::ALL),
    );
    frame.render_widget(help_table, rect);
}
