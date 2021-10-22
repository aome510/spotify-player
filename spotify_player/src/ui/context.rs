use std::sync::RwLockReadGuard;

use super::{construct_track_table_widget, Frame};
use crate::{state::*, ui::construct_list_widget};
use tui::{layout::*, widgets::*};

/// renders the context window which can be
/// - Current Playing: display the playing context of the current track
/// - Browsing: display the context of an arbitrary context
pub fn render_context_window(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
    title: &str,
) {
    let block = Block::default()
        .title(ui.theme.block_title_with_style(title))
        .borders(Borders::ALL);

    let player = state.player.read().unwrap();

    match player.context() {
        Some(context) => {
            frame.render_widget(block, rect);

            // render context description
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);
            let context_desc = Paragraph::new(context.description())
                .block(Block::default().style(ui.theme.context_desc()));
            frame.render_widget(context_desc, chunks[0]);

            match context {
                Context::Artist(_, ref tracks, ref albums, ref artists) => {
                    render_context_artist_widgets(
                        is_active,
                        frame,
                        ui,
                        state,
                        &player,
                        chunks[1],
                        (tracks, albums, artists),
                    );
                }
                Context::Playlist(_, ref tracks) => {
                    let track_table = construct_track_table_widget(
                        is_active,
                        ui,
                        state,
                        &player,
                        ui.filtered_items_by_search(tracks),
                    );

                    if let Some(state) = ui.window.track_table_state() {
                        frame.render_stateful_widget(track_table, rect, state)
                    }
                }
                Context::Album(_, ref tracks) => {
                    let track_table = construct_track_table_widget(
                        is_active,
                        ui,
                        state,
                        &player,
                        ui.filtered_items_by_search(tracks),
                    );

                    if let Some(state) = ui.window.track_table_state() {
                        frame.render_stateful_widget(track_table, rect, state)
                    }
                }
            }
        }
        None => {
            let desc = if player.context_id.is_none() {
                "Cannot infer the playing context from the current playback"
            } else {
                // context is not empty, but cannot get context data inside the player state
                // => still loading the context data
                "Loading..."
            };
            frame.render_widget(Paragraph::new(desc).block(block), rect);
        }
    }
}

/// renders the widgets for the artist context window, which includes
/// - A top track table
/// - An album list
/// - A related artist list
fn render_context_artist_widgets(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    player: &RwLockReadGuard<PlayerState>,
    rect: Rect,
    data: (&[Track], &[Album], &[Artist]),
) {
    let focus_state = match ui.window {
        WindowState::Artist(_, _, _, focus_state) => focus_state,
        _ => {
            return;
        }
    };
    let (tracks, albums, artists) = (
        ui.filtered_items_by_search(data.0),
        ui.filtered_items_by_search(data.1),
        ui.filtered_items_by_search(data.2),
    );

    let rect = {
        // render the top tracks table for artist context window

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
            .split(rect);

        let track_table = construct_track_table_widget(is_active, ui, state, &player, tracks);

        if let Some(state) = ui.window.track_table_state() {
            frame.render_stateful_widget(track_table, rect, state)
        }
        chunks[1]
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect);

    // construct album list widget
    let albums_list = {
        let album_items = albums
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            ui,
            album_items,
            "Albums",
            is_active && focus_state == ArtistFocusState::Albums,
            Some(Borders::TOP),
        )
    };

    // construct artist list widget
    let artists_list = {
        let artist_items = artists
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            ui,
            artist_items,
            "Related Artists",
            is_active && focus_state == ArtistFocusState::RelatedArtists,
            Some(Borders::TOP | Borders::LEFT),
        )
    };

    let (albums_list_state, artists_list_state) = match ui.window {
        WindowState::Artist(_, ref mut albums_list_state, ref mut artists_list_state, _) => {
            (albums_list_state, artists_list_state)
        }
        _ => unreachable!(),
    };

    frame.render_stateful_widget(albums_list, chunks[0], albums_list_state);
    frame.render_stateful_widget(artists_list, chunks[1], artists_list_state);
}
