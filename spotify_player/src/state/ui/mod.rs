use crate::{
    config::{self, Theme},
    key,
    ui::{self, Orientation},
    utils::filtered_items_from_query,
};

#[cfg(feature = "image")]
use crate::ui::cover_image::CoverImage;
#[cfg(feature = "image")]
use ratatui_image::picker::Picker;

use ratatui::layout::Rect;

pub type UIStateGuard<'a> = parking_lot::MutexGuard<'a, UIState>;

mod page;
mod popup;

pub use page::*;
pub use popup::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseTarget {
    ResumePause,
    LibraryWindow {
        focus: LibraryFocusState,
        first_item: usize,
        item_count: usize,
    },
    SearchInput,
    SearchWindow {
        focus: SearchFocusState,
        first_item: usize,
        item_count: usize,
    },
    ContextWindow {
        focus: Option<ArtistFocusState>,
        first_item: usize,
        item_count: usize,
    },
    BrowseWindow {
        first_item: usize,
        item_count: usize,
    },
    PopupList {
        first_item: usize,
        item_count: usize,
    },
    PlaylistCreateField(PlaylistCreateCurrentField),
    ConfirmAction(bool),
    ScrollablePage,
}

impl MouseTarget {
    pub fn is_available_with_focused_popup(self) -> bool {
        matches!(
            self,
            Self::ResumePause
                | Self::PopupList { .. }
                | Self::PlaylistCreateField(_)
                | Self::ConfirmAction(_)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseClickTarget {
    Library(LibraryFocusState, usize),
    Search(SearchFocusState, usize),
    Context(Option<ArtistFocusState>, usize),
    Browse(usize),
    Popup(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseArea {
    pub rect: Rect,
    pub target: MouseTarget,
}

impl MouseArea {
    pub fn contains(&self, column: u16, row: u16) -> bool {
        column >= self.rect.x
            && column < self.rect.x.saturating_add(self.rect.width)
            && row >= self.rect.y
            && row < self.rect.y.saturating_add(self.rect.height)
    }

    pub fn item_at(&self, row: u16) -> Option<usize> {
        let (first_item, item_count) = match self.target {
            MouseTarget::LibraryWindow {
                first_item,
                item_count,
                ..
            }
            | MouseTarget::SearchWindow {
                first_item,
                item_count,
                ..
            }
            | MouseTarget::ContextWindow {
                first_item,
                item_count,
                ..
            }
            | MouseTarget::BrowseWindow {
                first_item,
                item_count,
            }
            | MouseTarget::PopupList {
                first_item,
                item_count,
            } => (first_item, item_count),
            MouseTarget::ResumePause
            | MouseTarget::SearchInput
            | MouseTarget::PlaylistCreateField(_)
            | MouseTarget::ConfirmAction(_)
            | MouseTarget::ScrollablePage => return None,
        };

        let item = first_item + usize::from(row.saturating_sub(self.rect.y));
        (item < item_count).then_some(item)
    }
}

#[cfg(feature = "image")]
#[derive(Default)]
pub struct ImageRenderInfo {
    pub url: String,
    pub render_area: ratatui::layout::Rect,
    pub state: Option<CoverImage>,
}

#[cfg(feature = "image")]
impl std::fmt::Debug for ImageRenderInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageRenderInfo")
            .field("url", &self.url)
            .field("render_area", &self.render_area)
            .field("state", &self.state.is_some())
            .finish()
    }
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

    /// Interactive regions populated during the most recent render.
    pub mouse_areas: Vec<MouseArea>,

    last_mouse_click: Option<(MouseClickTarget, std::time::Instant)>,

    /// Count prefix for vim-style navigation (e.g., 5j, 10k)
    pub count_prefix: Option<usize>,

    #[cfg(feature = "image")]
    pub last_cover_image_render_info: ImageRenderInfo,

    #[cfg(feature = "image")]
    pub picker: Picker,
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
        self.popup = None;
        if let Some(current_page) = self.history.last() {
            if &page == current_page {
                return;
            }
        }
        self.history.push(page);
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
            Some(PopupState::Search { ref query }) => filtered_items_from_query(query, items),
            _ => items.iter().collect::<Vec<_>>(),
        }
    }

    pub fn register_mouse_click(&mut self, target: MouseClickTarget) -> bool {
        self.register_mouse_click_at(target, std::time::Instant::now())
    }

    fn register_mouse_click_at(
        &mut self,
        target: MouseClickTarget,
        now: std::time::Instant,
    ) -> bool {
        const DOUBLE_CLICK_MAX_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

        let is_double_click = self
            .last_mouse_click
            .is_some_and(|(last_target, last_time)| {
                last_target == target
                    && now
                        .checked_duration_since(last_time)
                        .is_some_and(|elapsed| elapsed <= DOUBLE_CLICK_MAX_DELAY)
            });
        self.last_mouse_click = if is_double_click {
            None
        } else {
            Some((target, now))
        };
        is_double_click
    }
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

            mouse_areas: Vec::new(),

            last_mouse_click: None,

            count_prefix: None,

            #[cfg(feature = "image")]
            last_cover_image_render_info: ImageRenderInfo::default(),

            // Will be reinitialize later in ui/mod.rs after init_ui()
            #[cfg(feature = "image")]
            picker: Picker::halfblocks(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_area(first_item: usize, item_count: usize) -> MouseArea {
        MouseArea {
            rect: Rect::new(10, 5, 20, 4),
            target: MouseTarget::LibraryWindow {
                focus: LibraryFocusState::Playlists,
                first_item,
                item_count,
            },
        }
    }

    #[test]
    fn mouse_area_contains_only_points_inside_rect() {
        let area = library_area(0, 4);

        assert!(area.contains(10, 5));
        assert!(area.contains(29, 8));
        assert!(!area.contains(9, 5));
        assert!(!area.contains(30, 8));
        assert!(!area.contains(10, 9));
    }

    #[test]
    fn mouse_area_maps_rows_to_scrolled_items() {
        let area = library_area(7, 20);

        assert_eq!(area.item_at(5), Some(7));
        assert_eq!(area.item_at(8), Some(10));
    }

    #[test]
    fn mouse_area_ignores_rows_after_last_item() {
        let area = library_area(7, 9);

        assert_eq!(area.item_at(6), Some(8));
        assert_eq!(area.item_at(7), None);
    }

    #[test]
    fn search_mouse_area_maps_rows_to_scrolled_items() {
        let area = MouseArea {
            rect: Rect::new(30, 10, 15, 5),
            target: MouseTarget::SearchWindow {
                focus: SearchFocusState::Albums,
                first_item: 4,
                item_count: 12,
            },
        };

        assert_eq!(area.item_at(10), Some(4));
        assert_eq!(area.item_at(14), Some(8));
    }

    #[test]
    fn popup_mouse_area_maps_only_rendered_items() {
        let area = MouseArea {
            rect: Rect::new(5, 20, 30, 4),
            target: MouseTarget::PopupList {
                first_item: 8,
                item_count: 10,
            },
        };

        assert_eq!(area.item_at(20), Some(8));
        assert_eq!(area.item_at(21), Some(9));
        assert_eq!(area.item_at(22), None);
    }

    #[test]
    fn focused_popup_masks_page_targets() {
        assert!(MouseTarget::ResumePause.is_available_with_focused_popup());
        assert!(MouseTarget::PopupList {
            first_item: 0,
            item_count: 1,
        }
        .is_available_with_focused_popup());
        assert!(
            MouseTarget::PlaylistCreateField(PlaylistCreateCurrentField::Desc)
                .is_available_with_focused_popup()
        );
        assert!(MouseTarget::ConfirmAction(true).is_available_with_focused_popup());
        assert!(!MouseTarget::SearchInput.is_available_with_focused_popup());
        assert!(!MouseTarget::ScrollablePage.is_available_with_focused_popup());
    }

    #[test]
    fn repeated_item_click_within_threshold_is_a_double_click() {
        let mut ui = UIState::default();
        let now = std::time::Instant::now();
        let target = MouseClickTarget::Search(SearchFocusState::Artists, 2);

        assert!(!ui.register_mouse_click_at(target, now));
        assert!(ui.register_mouse_click_at(target, now + std::time::Duration::from_millis(400)));
    }

    #[test]
    fn repeated_popup_item_click_is_a_double_click() {
        let mut ui = UIState::default();
        let now = std::time::Instant::now();
        let target = MouseClickTarget::Popup(3);

        assert!(!ui.register_mouse_click_at(target, now));
        assert!(ui.register_mouse_click_at(target, now + std::time::Duration::from_millis(250)));
    }

    #[test]
    fn different_or_slow_item_clicks_are_not_double_clicks() {
        let mut ui = UIState::default();
        let now = std::time::Instant::now();

        assert!(!ui.register_mouse_click_at(MouseClickTarget::Browse(1), now));
        assert!(!ui.register_mouse_click_at(
            MouseClickTarget::Browse(2),
            now + std::time::Duration::from_millis(100)
        ));
        assert!(!ui.register_mouse_click_at(
            MouseClickTarget::Browse(2),
            now + std::time::Duration::from_millis(700)
        ));
    }
}
