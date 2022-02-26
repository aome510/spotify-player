use crate::{config, key};

pub type UIStateGuard<'a> = parking_lot::MutexGuard<'a, UIState>;

mod page;
mod popup;

pub use page::*;
pub use popup::*;

/// Application's UI state
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,

    pub history: Vec<PageState>,
    pub popup: Option<PopupState>,

    // the rectangle representing the player's progress bar position,
    // which is mainly used to handle mouse click events (for track seeking)
    pub progress_bar_rect: tui::layout::Rect,
}

impl UIState {
    pub fn current_page(&self) -> &PageState {
        self.history.last().expect("History must not be empty")
    }

    pub fn current_page_mut(&mut self) -> &mut PageState {
        self.history.last_mut().expect("History must not be empty")
    }

    pub fn create_new_page(&mut self, page: PageState) {
        self.history.push(page);
        self.popup = None;
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: config::Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },

            history: vec![],
            popup: None,

            progress_bar_rect: tui::layout::Rect::default(),
        }
    }
}
