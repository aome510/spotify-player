use crate::{
    config::{self, Theme},
    key,
    ui::{self, Orientation},
};

pub type UIStateGuard<'a> = parking_lot::MutexGuard<'a, UIState>;

mod page;
mod popup;

use super::TracksId;

pub use page::*;
pub use popup::*;

#[derive(Default, Debug)]
#[cfg(feature = "image")]
pub struct ImageRenderInfo {
    pub url: String,
    pub render_area: ratatui::layout::Rect,
    /// indicates if the image is rendered
    pub rendered: bool,
}

/// Application's UI state
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,
    pub orientation: ui::Orientation,

    pub history: Vec<PageState>,
    pub popup: Option<PopupState>,

    /// The rectangle representing the playback progress bar,
    /// which is mainly used to handle mouse click events (for seeking command)
    pub playback_progress_bar_rect: ratatui::layout::Rect,

    #[cfg(feature = "image")]
    pub last_cover_image_render_info: ImageRenderInfo,
}

impl UIState {
    pub fn current_page(&self) -> &PageState {
        self.history.last().expect("non-empty history")
    }

    pub fn current_page_mut(&mut self) -> &mut PageState {
        self.history.last_mut().expect("non-empty history")
    }

    pub fn new_search_popup(&mut self) {
        self.current_page_mut().select(0);
        self.popup = Some(PopupState::Search {
            query: String::new(),
        });
    }

    pub fn new_page(&mut self, page: PageState) {
        self.history.push(page);
        self.popup = None;
    }

    pub fn new_radio_page(&mut self, uri: &str) {
        self.new_page(PageState::Context {
            id: None,
            context_page_type: ContextPageType::Browsing(super::ContextId::Tracks(TracksId::new(
                format!("radio:{uri}"),
                "Recommendations",
            ))),
            state: None,
        });
    }

    /// Return whether there exists a focused popup.
    ///
    /// Currently, only search popup is not focused when it's opened.
    pub fn has_focused_popup(&self) -> bool {
        match self.popup.as_ref() {
            None => false,
            Some(popup) => !matches!(popup, PopupState::Search { .. }),
        }
    }

    /// Get a list of items possibly filtered by a search query if exists a search popup
    pub fn search_filtered_items<'a, T: std::fmt::Display>(&self, items: &'a [T]) -> Vec<&'a T> {
        match self.popup {
            Some(PopupState::Search { ref query }) => {
                let query = query.to_lowercase();

                #[cfg(feature = "fzf")]
                return fuzzy_search_items(items, &query);

                #[cfg(not(feature = "fzf"))]
                items
                    .iter()
                    .filter(|t| {
                        if query.is_empty() {
                            true
                        } else {
                            let t = t.to_string().to_lowercase();
                            query
                                .split(' ')
                                .filter(|q| !q.is_empty())
                                .all(|q| t.contains(q))
                        }
                    })
                    .collect::<Vec<_>>()
            }
            _ => items.iter().collect::<Vec<_>>(),
        }
    }
}

#[cfg(feature = "fzf")]
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::layout::Rect;

#[cfg(feature = "fzf")]
fn fuzzy_search_items<'a, T: std::fmt::Display>(items: &'a [T], query: &str) -> Vec<&'a T> {
    let matcher = SkimMatcherV2::default();
    let mut result = items
        .iter()
        .filter_map(|t| {
            matcher
                .fuzzy(&t.to_string(), &query, false)
                .map(|(score, _)| (t, score))
        })
        .collect::<Vec<_>>();

    result.sort_by(|(_, a), (_, b)| b.cmp(a));
    result.into_iter().map(|(t, _)| t).collect::<Vec<_>>()
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },
            orientation: match crossterm::terminal::size() {
                Ok((columns, rows)) => ui::Orientation::from_size(columns, rows),
                Err(err) => {
                    tracing::warn!("Unable to get terminal size, error: {err:#}");
                    Orientation::default()
                }
            },

            history: vec![PageState::Library {
                state: LibraryPageUIState::new(),
            }],
            popup: None,

            playback_progress_bar_rect: Rect::default(),

            #[cfg(feature = "image")]
            last_cover_image_render_info: ImageRenderInfo::default(),
        }
    }
}
