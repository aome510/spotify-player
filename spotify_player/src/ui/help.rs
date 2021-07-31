use super::Frame;
use crate::config;
use crate::prelude::*;

const SHORTCUT_TABLE_N_COLUMNS: usize = 4;
const SHORTCUT_TABLE_CONSTRAINS: [Constraint; SHORTCUT_TABLE_N_COLUMNS] = [
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(25),
];

pub fn render_shortcuts_help_widget(frame: &mut Frame, matches: Vec<config::Keymap>, rect: Rect) {
    log::info!("{:?}", matches);
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
    .block(Block::default().title("Shortcuts").borders(Borders::ALL));
    frame.render_widget(help_table, rect);
}
