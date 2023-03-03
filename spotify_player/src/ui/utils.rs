use super::*;

pub fn construct_block<'a>(
    title: &str,
    theme: &config::Theme,
    state: &SharedState,
    borders: Option<Borders>,
) -> Block<'a> {
    let borders = borders.unwrap_or(Borders::ALL);
    let (borders, border_type) = match state.app_config.border_type {
        config::BorderType::None => (Borders::NONE, BorderType::Plain),
        config::BorderType::Plain => (borders, BorderType::Plain),
        config::BorderType::Rounded => (borders, BorderType::Rounded),
        config::BorderType::Double => (borders, BorderType::Double),
        config::BorderType::Thick => (borders, BorderType::Thick),
    };

    Block::default()
        .title(theme.block_title_with_style(title))
        .borders(borders)
        .border_style(theme.border())
        .border_type(border_type)
}

/// constructs a generic list widget
pub fn construct_list_widget<'a>(
    state: &SharedState,
    theme: &config::Theme,
    items: Vec<(String, bool)>,
    title: &str,
    is_active: bool,
    borders: Option<Borders>,
) -> (List<'a>, usize) {
    let n_items = items.len();

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
        .block(construct_block(title, theme, state, borders)),
        n_items,
    )
}

// adjust the `selected` position of a `ListState` if that position is out of index
fn adjust_list_state(state: &mut ListState, len: usize) {
    if let Some(p) = state.selected() {
        if p >= len {
            state.select(if len > 0 { Some(len - 1) } else { Some(0) });
        }
    }
}

pub fn render_list_window(
    frame: &mut Frame,
    widget: List,
    rect: Rect,
    len: usize,
    state: &mut ListState,
) {
    adjust_list_state(state, len);
    frame.render_stateful_widget(widget, rect, state);
}

// adjust the `selected` position of a `TableState` if that position is out of index
fn adjust_table_state(state: &mut TableState, len: usize) {
    if let Some(p) = state.selected() {
        if p >= len {
            state.select(if len > 0 { Some(len - 1) } else { Some(0) });
        }
    }
}

pub fn render_table_window(
    frame: &mut Frame,
    widget: Table,
    rect: Rect,
    len: usize,
    state: &mut TableState,
) {
    adjust_table_state(state, len);
    frame.render_stateful_widget(widget, rect, state);
}

pub fn render_loading_window(
    state: &SharedState,
    theme: &config::Theme,
    frame: &mut Frame,
    rect: Rect,
    title: &str,
) {
    frame.render_widget(
        Paragraph::new("Loading...").block(construct_block(title, theme, state, None)),
        rect,
    );
}
