use super::Frame;
use crate::{state::*, ui::construct_list_widget};
use {tui::layout::*, tui::widgets::*};

/// renders the search window showing the search results
/// of Spotify items (tracks, artists, albums, playlists) that match a given query
///
/// # Panic
/// This function will panic if the current UI's `PageState` is not `PageState::Searching`
pub fn render_search_window(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    // gets the current search query from UI's `PageState`
    let query = match ui.current_page() {
        PageState::Searching {
            ref current_query, ..
        } => current_query,
        _ => unreachable!(),
    };

    let focus_state = match ui.window {
        WindowState::Search { focus, .. } => focus,
        _ => {
            return;
        }
    };

    let data = state.data.read().unwrap();

    let search_results = data.caches.search.peek(query);

    let track_list = {
        let track_items = search_results
            .map(|s| {
                s.tracks
                    .iter()
                    .map(|a| (format!("{} - {}", a.name, a.artists_info()), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Tracks;

        construct_list_widget(
            ui,
            track_items,
            &format!("Tracks{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
    };

    let album_list = {
        let album_items = search_results
            .map(|s| {
                s.albums
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Albums;

        construct_list_widget(
            ui,
            album_items,
            &format!("Albums{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
    };

    let artist_list = {
        let artist_items = search_results
            .map(|s| {
                s.artists
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Artists;

        construct_list_widget(
            ui,
            artist_items,
            &format!("Artists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
    };

    let playlist_list = {
        let playlist_items = search_results
            .map(|s| {
                s.playlists
                    .iter()
                    .map(|a| (a.name.clone(), false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Playlists;

        construct_list_widget(
            ui,
            playlist_items,
            &format!("Playlists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
    };

    // renders borders with title
    let block = Block::default()
        .title(ui.theme.block_title_with_style("Search"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    // renders the query input box
    let rect = {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
            .split(rect);

        let is_active = is_active && focus_state == SearchFocusState::Input;

        frame.render_widget(
            Paragraph::new(query.clone()).style(ui.theme.selection_style(is_active)),
            chunks[0],
        );

        chunks[1]
    };

    // split the given `rect` layout into a 2x2 layout consiting of 4 chunks
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect)
        .into_iter()
        .flat_map(|rect| {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(rect)
        })
        .collect::<Vec<_>>();

    // get the mutable list states inside the UI's `WindowState`
    // to render the search window's sub-windows
    let (track_list_state, album_list_state, artist_list_state, playlist_list_state) =
        match ui.window {
            WindowState::Search {
                ref mut track_list,
                ref mut album_list,
                ref mut artist_list,
                ref mut playlist_list,
                ..
            } => (track_list, album_list, artist_list, playlist_list),
            _ => unreachable!(),
        };

    frame.render_stateful_widget(track_list, chunks[0], track_list_state);
    frame.render_stateful_widget(album_list, chunks[1], album_list_state);
    frame.render_stateful_widget(artist_list, chunks[2], artist_list_state);
    frame.render_stateful_widget(playlist_list, chunks[3], playlist_list_state);
}
