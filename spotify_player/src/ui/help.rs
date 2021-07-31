use super::Frame;
use crate::config;
use tui::{layout::*, widgets::*};

const SHORTCUT_TABLE_N_COLUMNS: usize = 4;
const SHORTCUT_TABLE_CONSTRAINS: [Constraint; SHORTCUT_TABLE_N_COLUMNS] = [
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(25),
];

/// renders a shortcuts help widget from a list of keymaps
pub fn render_shortcuts_help_widget(frame: &mut Frame, keymaps: Vec<config::Keymap>, rect: Rect) {
    log::info!("{:?}", keymaps);
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
