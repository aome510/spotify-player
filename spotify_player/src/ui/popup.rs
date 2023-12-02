use super::{utils::construct_and_render_block, *};
use crate::utils::format_duration;
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
/// The function returns a rectangle area to render the main layout and
/// a boolean value determining whether the focus should be placed in the main layout.
pub fn render_popup(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> (Rect, bool) {
    match ui.popup {
        None => (rect, true),
        Some(ref popup) => match popup {
            PopupState::Search { query } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(rect);

                let rect = construct_and_render_block(
                    "Search",
                    &ui.theme,
                    state,
                    Borders::ALL,
                    frame,
                    chunks[1],
                );

                frame.render_widget(Paragraph::new(format!("/{query}")), rect);
                (chunks[0], true)
            }
            PopupState::CommandHelp { .. } => {
                // the command help popup will cover the entire main layout
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(0)].as_ref())
                    .split(rect);

                render_commands_help_popup(frame, state, ui, chunks[0]);
                (chunks[1], false)
            }
            PopupState::Queue { .. } => {
                // the queue popup will cover the entire main layout
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(0)].as_ref())
                    .split(rect);

                render_queue_popup(frame, state, ui, chunks[0]);
                (chunks[1], false)
            }
            PopupState::ActionList(item, _) => {
                let rect = render_list_popup(
                    frame,
                    rect,
                    &format!("Actions on {}", item.name()),
                    item.actions_desc()
                        .into_iter()
                        .enumerate()
                        .map(|(id, d)| (format!("[{id}] {d}"), false))
                        .collect(),
                    item.n_actions() as u16 + 2, // 2 for top/bot paddings
                    state,
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

                let rect = render_list_popup(frame, rect, "Devices", items, 5, state, ui);
                (rect, false)
            }
            PopupState::ThemeList(themes, ..) => {
                let items = themes.iter().map(|t| (t.name.clone(), false)).collect();

                let rect = render_list_popup(frame, rect, "Themes", items, 7, state, ui);
                (rect, false)
            }
            PopupState::UserPlaylistList(action, _) => {
                let data = state.data.read();
                let playlists = match action {
                    PlaylistPopupAction::Browse => data.user_data.playlists.iter().collect(),
                    PlaylistPopupAction::AddTrack(_) => data.user_data.modifiable_playlists(),
                };
                let items = playlists
                    .into_iter()
                    .map(|p| (p.to_string(), false))
                    .collect();

                let rect = render_list_popup(frame, rect, "User Playlists", items, 10, state, ui);
                (rect, false)
            }
            PopupState::UserFollowedArtistList { .. } => {
                let items = state
                    .data
                    .read()
                    .user_data
                    .followed_artists
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect();

                let rect =
                    render_list_popup(frame, rect, "User Followed Artists", items, 7, state, ui);
                (rect, false)
            }
            PopupState::UserSavedAlbumList { .. } => {
                let items = state
                    .data
                    .read()
                    .user_data
                    .saved_albums
                    .iter()
                    .map(|a| (a.to_string(), false))
                    .collect();

                let rect = render_list_popup(frame, rect, "User Saved Albums", items, 7, state, ui);
                (rect, false)
            }
            PopupState::ArtistList(_, artists, ..) => {
                let items = artists.iter().map(|a| (a.to_string(), false)).collect();

                let rect = render_list_popup(frame, rect, "Artists", items, 5, state, ui);
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
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(length)].as_ref())
        .split(rect);

    let rect = construct_and_render_block(title, &ui.theme, state, Borders::ALL, frame, chunks[1]);
    let (list, len) = utils::construct_list_widget(&ui.theme, items, true);

    utils::render_list_window(
        frame,
        list,
        rect,
        len,
        ui.popup.as_mut().unwrap().list_state_mut().unwrap(),
    );

    chunks[0]
}

/// renders a shortcut help popup to show the available shortcuts based on user's inputs
pub fn render_shortcut_help_popup(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Rect {
    let input = &ui.input_key_sequence;

    // get the matches (keymaps) from the current key sequence input,
    // if there is at lease one match, render the shortcut help popup
    let matches = {
        if input.keys.is_empty() {
            vec![]
        } else {
            state
                .configs
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

        let rect = construct_and_render_block(
            "Shortcuts",
            &ui.theme,
            state,
            Borders::ALL,
            frame,
            chunks[1],
        );

        let help_table = Table::new(
            matches
                .into_iter()
                .map(|km| format!("{}: {:?}", km.key_sequence, km.command))
                .collect::<Vec<_>>()
                .chunks(SHORTCUT_TABLE_N_COLUMNS)
                .map(|c| Row::new(c.iter().map(|i| Cell::from(i.to_owned()))))
                .collect::<Vec<_>>(),
        )
        .widths(&SHORTCUT_TABLE_CONSTRAINS);

        frame.render_widget(help_table, rect);
        chunks[0]
    }
}

/// renders a command help popup listing all key shortcuts and corresponding descriptions
pub fn render_commands_help_popup(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    let rect = construct_and_render_block("Commands", &ui.theme, state, Borders::ALL, frame, rect);

    let mut map = BTreeMap::new();
    state
        .configs
        .keymap_config
        .keymaps
        .iter()
        .filter(|km| km.include_in_help_screen())
        .for_each(|km| {
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

    let scroll_offset = match ui.popup {
        Some(PopupState::CommandHelp {
            ref mut scroll_offset,
        }) => scroll_offset,
        _ => return,
    };
    // offset should not be greater than or equal the number of available commands
    if *scroll_offset >= map.len() {
        *scroll_offset = map.len() - 1
    }

    let help_table = Table::new(
        map.into_iter()
            .skip(*scroll_offset)
            .map(|(c, k)| {
                Row::new(vec![
                    Cell::from(format!("{c:?}")),
                    Cell::from(format!("[{k}]")),
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
    .widths(&COMMAND_TABLE_CONSTRAINTS);

    frame.render_widget(help_table, rect);
}

/// renders a queue popup listing everything in the queue
pub fn render_queue_popup(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    use rspotify::model::{FullEpisode, FullTrack, PlayableItem};
    fn get_playable_name(item: &PlayableItem) -> String {
        match item {
            PlayableItem::Track(FullTrack { ref name, .. }) => name,
            PlayableItem::Episode(FullEpisode { ref name, .. }) => name,
        }
        .to_string()
    }
    fn get_playable_artists(item: &PlayableItem) -> String {
        match item {
            PlayableItem::Track(FullTrack { ref artists, .. }) => artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            PlayableItem::Episode(FullEpisode { .. }) => String::new(),
        }
    }
    fn get_playable_duration(item: &PlayableItem) -> String {
        match item {
            PlayableItem::Track(FullTrack { ref duration, .. }) => format_duration(duration),
            PlayableItem::Episode(FullEpisode { ref duration, .. }) => format_duration(duration),
        }
    }

    let rect = construct_and_render_block("Queue", &ui.theme, state, Borders::ALL, frame, rect);

    let scroll_offset = match ui.popup {
        Some(PopupState::Queue {
            ref mut scroll_offset,
        }) => scroll_offset,
        _ => return,
    };

    // Minimize the time we have a lock on the player state
    let queue_table = {
        let player_state = state.player.read();
        let queue = match player_state.queue {
            Some(ref q) => &q.queue,
            None => return,
        };

        // offset should not be greater than or equal the number of items in queue
        if !queue.is_empty() && *scroll_offset >= queue.len() {
            *scroll_offset = queue.len() - 1
        }
        Table::new(
            queue
                .iter()
                .enumerate()
                .skip(*scroll_offset)
                .map(|(i, x)| {
                    Row::new(vec![
                        Cell::from(format!("{}", i + 1)),
                        Cell::from(get_playable_name(x)),
                        Cell::from(get_playable_artists(x)),
                        Cell::from(get_playable_duration(x)),
                    ])
                })
                .collect::<Vec<_>>(),
        )
        .header(
            Row::new(vec![
                Cell::from("#"),
                Cell::from("Title"),
                Cell::from("Artists"),
                Cell::from("Duration"),
            ])
            .style(ui.theme.table_header()),
        )
        .widths(&[
            Constraint::Percentage(5),
            Constraint::Percentage(40),
            Constraint::Percentage(35),
            Constraint::Percentage(20),
        ])
    };

    frame.render_widget(queue_table, rect);
}
