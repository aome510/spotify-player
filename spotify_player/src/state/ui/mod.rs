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

    /// Gets a list of items possibly filtered by a search query if exists a search popup
    pub fn search_filtered_items<'a, T: std::fmt::Display>(&self, items: &'a [T]) -> Vec<&'a T> {
        match self.popup {
            Some(PopupState::Search { ref query }) => items
                .iter()
                .filter(|t| Self::is_match(&t.to_string().to_lowercase(), &query.to_lowercase()))
                .collect::<Vec<_>>(),
            _ => items.iter().collect::<Vec<_>>(),
        }
    }

    /// checks if a string matches a given query
    fn is_match(s: &str, query: &str) -> bool {
        query
            .split(' ')
            .fold(true, |acc, cur| acc & s.contains(cur))
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: config::Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },

            history: vec![PageState::Library {
                state: LibraryPageUIState::new(),
            }],
            popup: None,

            progress_bar_rect: tui::layout::Rect::default(),
        }
    }
}
