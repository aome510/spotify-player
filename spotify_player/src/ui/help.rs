use super::Frame;
use crate::{config, state};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use tui::{layout::*, style::*, widgets::*};

const SHORTCUT_TABLE_N_COLUMNS: usize = 4;
const SHORTCUT_TABLE_CONSTRAINS: [Constraint; SHORTCUT_TABLE_N_COLUMNS] = [
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(25),
];
const COMMAND_TABLE_CONSTRAINTS: [Constraint; 2] =
    [Constraint::Percentage(30), Constraint::Percentage(70)];

/// renders a shortcuts help widget from a list of keymaps
pub fn render_shortcuts_help_widget(frame: &mut Frame, keymaps: Vec<config::Keymap>, rect: Rect) {
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
    .block(Block::default().title("Shortcuts").borders(Borders::ALL));
    frame.render_widget(help_table, rect);
}

pub fn render_commands_help_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let state = state.read().unwrap();
    let mut map = BTreeMap::new();
    state.keymap_config.keymaps.iter().for_each(|km| {
        let v = map.entry(km.command.clone());
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
    let help_table = Table::new(
        map.into_iter()
            .map(|(c, k)| {
                Row::new(vec![
                    Cell::from(format!("{:?}", c)),
                    Cell::from(format!("[{}]", k)),
                ])
            })
            .collect::<Vec<_>>(),
    )
    .header(
        Row::new(vec![Cell::from("Command"), Cell::from("Keys")])
            .style(Style::default().fg(Color::Yellow)),
    )
    .widths(&COMMAND_TABLE_CONSTRAINTS)
    .block(Block::default().title("Shortcuts").borders(Borders::ALL));
    frame.render_widget(help_table, rect);
}
