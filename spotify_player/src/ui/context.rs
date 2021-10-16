use super::Frame;
use crate::{state::*, ui::construct_list_widget, utils};
use std::sync::RwLockReadGuard;
use tui::{layout::*, style::*, widgets::*};

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
                    render_context_artist_widget(
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
                    render_context_track_table_widget(
                        is_active,
                        frame,
                        ui,
                        state,
                        &player,
                        chunks[1],
                        ui.search_filtered_items(tracks),
                    );
                }
                Context::Album(_, ref tracks) => {
                    render_context_track_table_widget(
                        is_active,
                        frame,
                        ui,
                        state,
                        &player,
                        chunks[1],
                        ui.search_filtered_items(tracks),
                    );
                }
                Context::Unknown(_) => {}
            }
        }
        None => {
            let desc = if player.context_uri.is_empty() {
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

fn render_context_artist_widget(
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
        ui.search_filtered_items(data.0),
        ui.search_filtered_items(data.1),
        ui.search_filtered_items(data.2),
    );

    let rect = {
        // render the top tracks table for artist context window

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
            .split(rect);
        render_context_track_table_widget(
            is_active && focus_state == ArtistFocusState::TopTracks,
            frame,
            ui,
            state,
            player,
            chunks[0],
            tracks,
        );
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

/// constructs a track table widget then renders it
fn render_context_track_table_widget(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    player: &RwLockReadGuard<PlayerState>,
    rect: Rect,
    tracks: Vec<&Track>,
) {
    let track_table = {
        let mut playing_track_uri = "".to_string();
        let mut active_desc = "";
        if let Some(ref playback) = player.playback {
            if let Some(rspotify::model::PlayingItem::Track(ref track)) = playback.item {
                playing_track_uri = track.uri.clone();
                active_desc = if !playback.is_playing { "⏸" } else { "▶" };
            }
        }

        let item_max_len = state.app_config.track_table_item_max_len;
        let rows = tracks
            .into_iter()
            .enumerate()
            .map(|(id, t)| {
                let (id, style) = if playing_track_uri == t.uri {
                    (active_desc.to_string(), ui.theme.current_active())
                } else {
                    ((id + 1).to_string(), Style::default())
                };
                Row::new(vec![
                    Cell::from(id),
                    Cell::from(utils::truncate_string(t.name.clone(), item_max_len)),
                    Cell::from(utils::truncate_string(t.artists_info(), item_max_len)),
                    Cell::from(utils::truncate_string(t.album.name.clone(), item_max_len)),
                    Cell::from(utils::format_duration(t.duration)),
                ])
                .style(style)
            })
            .collect::<Vec<_>>();

        Table::new(rows)
            .header(
                Row::new(vec![
                    Cell::from("#"),
                    Cell::from("Track"),
                    Cell::from("Artists"),
                    Cell::from("Album"),
                    Cell::from("Duration"),
                ])
                .style(ui.theme.context_tracks_table_header()),
            )
            .block(Block::default())
            .widths(&[
                Constraint::Length(4),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(10),
            ])
            .highlight_style(ui.theme.selection_style(is_active))
    };

    if let Some(state) = ui.window.track_table_state() {
        frame.render_stateful_widget(track_table, rect, state)
    }
}
