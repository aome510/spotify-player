use crate::prelude::*;

const SHORTCUT_TABLE_N_COLUMNS: usize = 3;

pub fn get_sort_shortcuts() -> Vec<(impl std::fmt::Display, impl std::fmt::Display)> {
    vec![
        ("q", "Sort by track ascending"),
        ("Q", "Sort by track descending"),
        ("w", "Sort by album ascending"),
        ("W", "Sort by album descending"),
        ("e", "Sort by artists ascending"),
        ("E", "Sort by artists descending"),
        ("r", "Sort by added date ascending"),
        ("R", "Sort by added date descending"),
        ("t", "Sort by duration ascending"),
        ("T", "Sort by duration descending"),
    ]
}

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
