use super::Frame;
use crate::{state::*, ui::construct_list_widget};
use {tui::layout::*, tui::widgets::*};

/// renders the search window showing the search results
/// of Spotify items (tracks, artists, albums, playlists) that match a given query
///
/// # Panic
/// This function will panic if the current UI's `PageState` is not `PageState::Searching`
pub fn render_search_window(is_active: bool, frame: &mut Frame, ui: &mut UIStateGuard, rect: Rect) {
    // gets the search query from UI's `PageState`
    let (query, search_results) = match ui.current_page() {
        PageState::Searching(ref query, ref search_results) => (query, search_results),
        _ => unreachable!(),
    };

    let focus_state = match ui.window {
        WindowState::Search(_, _, _, _, focus) => focus,
        _ => {
            return;
        }
    };

    let tracks_list = {
        let track_items = search_results
            .tracks
            .iter()
            .map(|a| (format!("{} - {}", a.name, a.artists_info()), false))
            .collect::<Vec<_>>();

        let is_active = is_active && focus_state == SearchFocusState::Tracks;

        construct_list_widget(
            ui,
            track_items,
            &format!("Tracks{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
    };

    let albums_list = {
        let album_items = search_results
            .albums
            .iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        let is_active = is_active && focus_state == SearchFocusState::Albums;

        construct_list_widget(
            ui,
            album_items,
            &format!("Albums{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP),
        )
    };

    let artists_list = {
        let artist_items = search_results
            .artists
            .iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        let is_active = is_active && focus_state == SearchFocusState::Artists;

        construct_list_widget(
            ui,
            artist_items,
            &format!("Artists{}", if is_active { " [*]" } else { "" }),
            is_active,
            Some(Borders::TOP | Borders::RIGHT),
        )
    };

    let playlists_list = {
        let playlist_items = search_results
            .playlists
            .iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

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

    // get the list states inside the UI's `WindowState` to render the search window's sub-windows
    let (tracks_list_state, albums_list_state, artists_list_state, playlists_list_state) =
        match ui.window {
            WindowState::Search(
                ref mut tracks_list_state,
                ref mut albums_list_state,
                ref mut artists_list_state,
                ref mut playlists_list_state,
                _,
            ) => (
                tracks_list_state,
                albums_list_state,
                artists_list_state,
                playlists_list_state,
            ),
            _ => unreachable!(),
        };

    frame.render_stateful_widget(tracks_list, chunks[0], tracks_list_state);
    frame.render_stateful_widget(albums_list, chunks[1], albums_list_state);
    frame.render_stateful_widget(artists_list, chunks[2], artists_list_state);
    frame.render_stateful_widget(playlists_list, chunks[3], playlists_list_state);
}
