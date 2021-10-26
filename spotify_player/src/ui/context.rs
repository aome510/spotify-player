use super::{render_track_table_widget, Frame};
use crate::{state::*, ui::construct_list_widget};
use tui::{layout::*, widgets::*};

/// renders the context window which can be
/// - Current Playing: display the playing context of the current track
/// - Browsing: display the context of an arbitrary context
pub fn render_context_window(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    rect: Rect,
    title: &str,
) {
    let block = Block::default()
        .title(state.ui.lock().theme.block_title_with_style(title))
        .borders(Borders::ALL);

    let data = state.data.read();

    match state.player.read().context(&data.caches) {
        Some(context) => {
            frame.render_widget(block, rect);

            // render context description
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);
            let context_desc = Paragraph::new(context.description())
                .block(Block::default().style(state.ui.lock().theme.context_desc()));
            frame.render_widget(context_desc, chunks[0]);

            match context {
                Context::Artist {
                    top_tracks,
                    albums,
                    related_artists,
                    ..
                } => {
                    render_context_artist_widgets(
                        is_active,
                        frame,
                        state,
                        chunks[1],
                        (top_tracks, albums, related_artists),
                    );
                }
                Context::Playlist { tracks, .. } => {
                    render_track_table_widget(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        state.filtered_items_by_search(tracks),
                    );
                }
                Context::Album { tracks, .. } => {
                    render_track_table_widget(
                        frame,
                        chunks[1],
                        is_active,
                        state,
                        state.filtered_items_by_search(tracks),
                    );
                }
            }
        }
        None => {
            let desc = if state.player.read().context_id.is_none() {
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
    state: &SharedState,
    rect: Rect,
    data: (&[Track], &[Album], &[Artist]),
) {
    let (tracks, albums, artists) = (
        state.filtered_items_by_search(data.0),
        state.filtered_items_by_search(data.1),
        state.filtered_items_by_search(data.2),
    );

    let focus_state = match state.ui.lock().window {
        WindowState::Artist { focus, .. } => focus,
        _ => {
            return;
        }
    };

    let rect = {
        // render the top tracks table for artist context window

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
            .split(rect);

        render_track_table_widget(
            frame,
            chunks[0],
            is_active && focus_state == ArtistFocusState::TopTracks,
            state,
            tracks,
        );

        chunks[1]
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect);

    // construct album list widget
    let album_list = {
        let album_items = albums
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            state,
            album_items,
            "Albums",
            is_active && focus_state == ArtistFocusState::Albums,
            Some(Borders::TOP),
        )
    };

    // construct artist list widget
    let artist_list = {
        let artist_items = artists
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        construct_list_widget(
            state,
            artist_items,
            "Related Artists",
            is_active && focus_state == ArtistFocusState::RelatedArtists,
            Some(Borders::TOP | Borders::LEFT),
        )
    };

    let mut ui = state.ui.lock();
    let (album_list_state, artist_list_state) = match ui.window {
        WindowState::Artist {
            ref mut album_list,
            ref mut related_artist_list,
            ..
        } => (album_list, related_artist_list),
        _ => unreachable!(),
    };

    frame.render_stateful_widget(album_list, chunks[0], album_list_state);
    frame.render_stateful_widget(artist_list, chunks[1], artist_list_state);
}
