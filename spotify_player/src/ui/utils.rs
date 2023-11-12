use super::*;

/// Construct and render a block.
///
/// This function should only be used to render a window's borders and its title.
/// It returns the rectangle to render the inner widgets inside the block.
pub fn construct_and_render_block(
    title: &str,
    theme: &config::Theme,
    state: &SharedState,
    borders: Borders,
    frame: &mut Frame,
    rect: Rect,
) -> Rect {
    let (borders, border_type) = match state.configs.app_config.border_type {
        config::BorderType::Hidden => (borders, BorderType::Plain),
        config::BorderType::Plain => (borders, BorderType::Plain),
        config::BorderType::Rounded => (borders, BorderType::Rounded),
        config::BorderType::Double => (borders, BorderType::Double),
        config::BorderType::Thick => (borders, BorderType::Thick),
    };

    let mut block = Block::default()
        .title(theme.block_title_with_style(title))
        .borders(borders)
        .border_style(theme.border())
        .border_type(border_type);

    let inner_rect = block.inner(rect);

    // Handle `BorderType::Hidden` after determining the inner rectangle
    if state.configs.app_config.border_type == config::BorderType::Hidden {
        block = block.borders(Borders::NONE);
    }

    frame.render_widget(block, rect);
    inner_rect
}

/// constructs a generic list widget
pub fn construct_list_widget<'a>(
    theme: &config::Theme,
    items: Vec<(String, bool)>,
    is_active: bool,
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
        .highlight_style(theme.selection_style(is_active)),
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
    let rect = construct_and_render_block(title, theme, state, Borders::ALL, frame, rect);
    frame.render_widget(Paragraph::new("Loading..."), rect);
}
