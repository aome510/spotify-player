use super::Frame;
use crate::{config, state::*};
use std::collections::{btree_map::Entry, BTreeMap};
use tui::{layout::*, widgets::*};

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

/// renders a shortcut help window to show the available shortcuts
/// based on user's inputs
pub fn render_shortcut_help_window(
    frame: &mut Frame,
    ui: &UIStateGuard,
    state: &SharedState,
    rect: Rect,
) -> Rect {
    let input = &ui.input_key_sequence;
    // render the shortcuts help table if needed
    let matches = if input.keys.is_empty() {
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
    };

    if matches.is_empty() {
        rect
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
            .split(rect);
        render_shortcuts_help_window(matches, frame, ui, chunks[1]);
        chunks[0]
    }
}

/// renders a command help window listing all key shortcuts and corresponding descriptions
pub fn render_commands_help_window(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    let offset = match ui.popup {
        PopupState::CommandHelp(ref mut offset) => offset,
        _ => unreachable!(),
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
        .style(ui.theme.context_tracks_table_header()),
    )
    .widths(&COMMAND_TABLE_CONSTRAINTS)
    .block(
        Block::default()
            .title(ui.theme.block_title_with_style("Commands"))
            .borders(Borders::ALL),
    );
    frame.render_widget(help_table, rect);
}

/// renders a shortcuts help widget from a list of keymaps
fn render_shortcuts_help_window(
    keymaps: Vec<config::Keymap>,
    frame: &mut Frame,
    ui: &UIStateGuard,
    rect: Rect,
) {
    let help_table = Table::new(
        keymaps
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
    frame.render_widget(help_table, rect);
}
