use super::{
    config, Block, BorderType, Borders, Frame, List, ListItem, ListState, Rect, Span, Style, Table,
    TableState,
};
use unicode_bidi::BidiInfo;

/// Construct and render a block.
///
/// This function should only be used to render a window's borders and its title.
/// It returns the rectangle to render the inner widgets inside the block.
pub fn construct_and_render_block(
    title: &str,
    theme: &config::Theme,
    borders: Borders,
    frame: &mut Frame,
    rect: Rect,
) -> Rect {
    let mut title = title.to_string();

    let configs = config::get_config();

    let (borders, border_type) = match configs.app_config.border_type {
        config::BorderType::Hidden | config::BorderType::Plain => (borders, BorderType::Plain),
        config::BorderType::Rounded => (borders, BorderType::Rounded),
        config::BorderType::Double => (borders, BorderType::Double),
        config::BorderType::Thick => (borders, BorderType::Thick),
    };

    let mut block = Block::default()
        .borders(borders)
        .border_style(theme.border())
        .border_type(border_type);

    let inner_rect = block.inner(rect);

    // Handle `BorderType::Hidden` after determining the inner rectangle
    // `Hidden` border can be done by setting the borders to be `NONE`.
    // NOTE: we want to handle the border after the inner rectangle computation,
    // so that paddings between windows are properly determined.
    if configs.app_config.border_type == config::BorderType::Hidden {
        block = block.borders(Borders::NONE);
        // add padding to the title to ensure the inner text is aligned with the title
        title = format!(" {title}");
    }

    // Set `title` for the block
    block = block.title(Span::styled(title, theme.block_title()));

    frame.render_widget(block, rect);
    inner_rect
}

/// Construct a generic list widget
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
        .highlight_style(theme.selection(is_active)),
        n_items,
    )
}

/// adjust the `selected` position of a `ListState` if that position is invalid
fn adjust_list_state(state: &mut ListState, len: usize) {
    if let Some(p) = state.selected() {
        if p >= len {
            state.select(if len > 0 { Some(len - 1) } else { Some(0) });
        }
    } else if len > 0 {
        state.select(Some(0));
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

/// adjust the `selected` position of a `TableState` if that position is invalid
fn adjust_table_state(state: &mut TableState, len: usize) {
    if let Some(p) = state.selected() {
        if p >= len {
            state.select(if len > 0 { Some(len - 1) } else { Some(0) });
        }
    } else if len > 0 {
        state.select(Some(0));
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

/// Convert a string to a bidirectional string.
/// Used to handle RTL text properly in the UI.
pub fn to_bidi_string(s: &str) -> String {
    let bidi_info = BidiInfo::new(s, None);

    let bidi_string = if bidi_info.has_rtl() && !bidi_info.paragraphs.is_empty() {
        bidi_info
            .reorder_line(&bidi_info.paragraphs[0], 0..s.len())
            .into_owned()
    } else {
        s.to_string()
    };

    bidi_string
}

/// formats genres depending on the number of genres and `genre_num`
///
/// Examples for `genre_num = 2`
/// - 1 genre: "genre1"
/// - 2 genres: "genre1, genre2"
/// - \>= 3 genres: "genre1, genre2, ..."
pub fn format_genres(genres: &[String], genre_num: u8) -> String {
    let mut genre_str = String::with_capacity(64);

    if genre_num > 0 {
        for i in 0..genres.len() {
            genre_str.push_str(&genres[i]);

            if i + 1 != genres.len() {
                genre_str.push_str(", ");

                if i + 1 >= genre_num as usize {
                    genre_str.push_str("...");
                    break;
                }
            }
        }
    }

    genre_str
}
