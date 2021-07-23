use crate::prelude::*;

const SHORTCUT_TABLE_N_COLUMNS: usize = 3;

pub fn get_shortcut_table<'a, X, Y>(items: Vec<(X, Y)>) -> Table<'a>
where
    X: std::fmt::Display,
    Y: std::fmt::Display,
{
    Table::new(
        items
            .into_iter()
            .map(|i| format!("{}: {}", i.0, i.1))
            .collect::<Vec<_>>()
            .chunks(SHORTCUT_TABLE_N_COLUMNS)
            .map(|c| Row::new(c.iter().map(|i| Cell::from(i.to_owned()))))
            .collect::<Vec<_>>(),
    )
    .widths(&[
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
    ])
    .block(Block::default().title("Shortcuts").borders(Borders::ALL))
}
