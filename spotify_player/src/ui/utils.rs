use super::*;

/// constructs a generic list widget
pub fn construct_list_widget<'a>(
    theme: &config::Theme,
    items: Vec<(String, bool)>,
    title: &str,
    is_active: bool,
    borders: Option<Borders>,
) -> (List<'a>, usize) {
    let n_items = items.len();
    let borders = borders.unwrap_or(Borders::ALL);

    (
        List::new(
            items
                .into_iter()
                .map(|(s, is_active)| {
                    ListItem::new(s).style(if is_active {
                        theme.current_playing()
                    } else {
                        Style::default()
                    })
                })
                .collect::<Vec<_>>(),
        )
        .highlight_style(theme.selection_style(is_active))
        .block(
            Block::default()
                .title(theme.block_title_with_style(title))
                .borders(borders),
        ),
        n_items,
    )
}
