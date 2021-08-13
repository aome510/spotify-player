use super::Frame;
use crate::{state::*, utils};
use tui::{layout::*, style::*, widgets::*};

pub fn render_context_widget(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    let player = state.player.read().unwrap();
    // context widget box border
    let block = Block::default()
        .title(ui.theme.block_title_with_style("Context"))
        .borders(Borders::ALL);

    match player.get_context() {
        Some(context) => {
            frame.render_widget(block, rect);

            // context description
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(rect);
            let context_desc = Paragraph::new(context.get_description())
                .block(Block::default().style(ui.theme.context_desc()));
            frame.render_widget(context_desc, chunks[0]);

            match context {
                Context::Artist(_, ref tracks, ref albums, ref artists) => {
                    render_context_album_widget(
                        is_active,
                        frame,
                        ui,
                        state,
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
                        chunks[1],
                        ui.get_search_filtered_items(tracks),
                    );
                }
                Context::Album(_, ref tracks) => {
                    render_context_track_table_widget(
                        is_active,
                        frame,
                        ui,
                        state,
                        chunks[1],
                        ui.get_search_filtered_items(tracks),
                    );
                }
                Context::Unknown(_) => {}
            }
        }
        None => {
            let loading_desc = Paragraph::new("Loading...").block(block);
            frame.render_widget(loading_desc, rect);
        }
    }
}

fn render_context_album_widget(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
    data: (&[Track], &[Album], &[Artist]),
) {
    let focus_state = match ui.context {
        ContextState::Artist(_, _, _, focus_state) => focus_state,
        _ => {
            return;
        }
    };
    let (tracks, albums, artists) = (
        ui.get_search_filtered_items(data.0),
        ui.get_search_filtered_items(data.1),
        ui.get_search_filtered_items(data.2),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
        .split(rect);
    render_context_track_table_widget(
        is_active && focus_state == ArtistFocusState::TopTracks,
        frame,
        ui,
        state,
        chunks[0],
        tracks,
    );

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    let current_album = state
        .player
        .read()
        .unwrap()
        .get_current_playing_track()
        .map(|t| t.album.name.clone())
        .unwrap_or_default();

    let albums_list = List::new(
        albums
            .into_iter()
            .map(|a| {
                ListItem::new(a.name.clone()).style(if a.name == current_album {
                    ui.theme.current_active()
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>(),
    )
    .highlight_style(
        ui.theme
            .selection_style(is_active && focus_state == ArtistFocusState::Albums),
    )
    .block(
        Block::default()
            .borders(Borders::TOP)
            .title(ui.theme.block_title_with_style("Albums")),
    );
    let artists_list = List::new(
        artists
            .into_iter()
            .map(|a| ListItem::new(a.name.clone()))
            .collect::<Vec<_>>(),
    )
    .highlight_style(
        ui.theme
            .selection_style(is_active && focus_state == ArtistFocusState::RelatedArtists),
    )
    .block(
        Block::default()
            .borders(Borders::TOP | Borders::LEFT)
            .title(ui.theme.block_title_with_style("Related Artists")),
    );

    let (albums_list_state, artists_list_state) = match ui.context {
        ContextState::Artist(_, ref mut albums_list_state, ref mut artists_list_state, _) => {
            (albums_list_state, artists_list_state)
        }
        _ => unreachable!(),
    };
    frame.render_stateful_widget(albums_list, chunks[0], albums_list_state);
    frame.render_stateful_widget(artists_list, chunks[1], artists_list_state);
}

fn render_context_track_table_widget(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
    tracks: Vec<&Track>,
) {
    let track_table = {
        let mut playing_track_uri = "".to_string();
        if let Some(ref playback) = state.player.read().unwrap().playback {
            if let Some(rspotify::model::PlayingItem::Track(ref track)) = playback.item {
                playing_track_uri = track.uri.clone();
            }
        }

        let item_max_len = state.app_config.track_table_item_max_len;
        let rows = tracks
            .into_iter()
            .enumerate()
            .map(|(id, t)| {
                let (id, style) = if playing_track_uri == t.uri {
                    ("▶".to_string(), ui.theme.current_active())
                } else {
                    ((id + 1).to_string(), Style::default())
                };
                Row::new(vec![
                    Cell::from(id),
                    Cell::from(utils::truncate_string(t.name.clone(), item_max_len)),
                    Cell::from(utils::truncate_string(t.get_artists_info(), item_max_len)),
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
                Constraint::Percentage(3),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(7),
            ])
            .highlight_style(ui.theme.selection_style(is_active))
    };

    if let Some(state) = ui.context.get_track_table_state() {
        frame.render_stateful_widget(track_table, rect, state)
    }
}
