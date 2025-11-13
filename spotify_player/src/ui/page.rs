use std::{
    collections::{btree_map::Entry, BTreeMap},
    fmt::Display,
};

use chrono_humanize::HumanTime;
use ratatui::text::Line;

use crate::{state::Episode, utils::format_duration};

use super::{
    config, utils, utils::construct_and_render_block, Album, Artist, ArtistFocusState, Borders,
    BrowsePageUIState, Cell, Constraint, Context, ContextPageUIState, DataReadGuard, Frame, Id,
    Layout, LibraryFocusState, MutableWindowState, Orientation, PageState, Paragraph,
    PlaylistFolderItem, Rect, Row, SearchFocusState, SharedState, Style, Table, Track,
    UIStateGuard,
};
use crate::state::BidiDisplay;
use crate::ui::utils::to_bidi_string;

const COMMAND_TABLE_CONSTRAINTS: [Constraint; 3] = [
    Constraint::Percentage(25),
    Constraint::Percentage(25),
    Constraint::Percentage(50),
];

// UI codes to render a page.
// A `render_*_page` function should follow (not strictly) the below steps
// 1. get data from the application's states
// 2. construct the page's layout
// 3. construct the page's widgets
// 4. render the widgets

pub fn render_search_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    fn search_items<T: Display>(items: &[T]) -> Vec<(String, bool)> {
        items
            .iter()
            .map(|i| (to_bidi_string(&i.to_string()), false))
            .collect()
    }

    // 1. Get data
    let data = state.data.read();

    let (focus_state, current_query, line_input) = match ui.current_page() {
        PageState::Search {
            state,
            current_query,
            line_input,
        } => (state.focus, current_query, line_input),
        _ => return,
    };

    let search_results = data.caches.search.get(current_query);

    // 2. Construct the page's layout
    let rect = construct_and_render_block("Search", &ui.theme, Borders::ALL, frame, rect);

    // search input's layout
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(0)]).split(rect);
    let search_input_rect = chunks[0];
    let rect = chunks[1];

    // track/album/artist/playlist/show/episode search results layout
    let chunks = match ui.orientation {
        // 1x6
        Orientation::Vertical => {
            let constraints = if focus_state == SearchFocusState::Input {
                [Constraint::Ratio(1, 6); 6]
            } else {
                let mut constraints = [Constraint::Percentage(15); 6];
                constraints[focus_state as usize - 1] = Constraint::Percentage(25);
                constraints
            };

            Layout::vertical(constraints).split(rect)
        }
        // 2x3
        Orientation::Horizontal => Layout::vertical([Constraint::Ratio(1, 3); 3])
            .split(rect)
            .iter()
            .flat_map(|rect| {
                Layout::horizontal([Constraint::Ratio(1, 2); 2])
                    .split(*rect)
                    .to_vec()
            })
            .collect(),
    };

    let track_rect = construct_and_render_block(
        "Tracks",
        &ui.theme,
        if ui.orientation == Orientation::Horizontal {
            Borders::TOP | Borders::RIGHT
        } else {
            Borders::TOP
        },
        frame,
        chunks[0],
    );
    let album_rect =
        construct_and_render_block("Albums", &ui.theme, Borders::TOP, frame, chunks[1]);
    let artist_rect = construct_and_render_block(
        "Artists",
        &ui.theme,
        if ui.orientation == Orientation::Horizontal {
            Borders::TOP | Borders::RIGHT
        } else {
            Borders::TOP
        },
        frame,
        chunks[2],
    );
    let playlist_rect =
        construct_and_render_block("Playlists", &ui.theme, Borders::TOP, frame, chunks[3]);
    let show_rect = construct_and_render_block(
        "Shows",
        &ui.theme,
        if ui.orientation == Orientation::Horizontal {
            Borders::TOP | Borders::RIGHT
        } else {
            Borders::TOP
        },
        frame,
        chunks[4],
    );
    let episode_rect =
        construct_and_render_block("Episodes", &ui.theme, Borders::TOP, frame, chunks[5]);

    // 3. Construct the page's widgets
    let (track_list, n_tracks) = {
        let track_items = search_results
            .map(|s| search_items(&s.tracks))
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Tracks;

        utils::construct_list_widget(&ui.theme, track_items, is_active)
    };

    let (album_list, n_albums) = {
        let album_items = search_results
            .map(|s| search_items(&s.albums))
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Albums;

        utils::construct_list_widget(&ui.theme, album_items, is_active)
    };

    let (artist_list, n_artists) = {
        let artist_items = search_results
            .map(|s| search_items(&s.artists))
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Artists;

        utils::construct_list_widget(&ui.theme, artist_items, is_active)
    };

    let (playlist_list, n_playlists) = {
        let playlist_items = search_results
            .map(|s| search_items(&s.playlists))
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Playlists;

        utils::construct_list_widget(&ui.theme, playlist_items, is_active)
    };

    let (show_list, n_shows) = {
        let show_items = search_results
            .map(|s| search_items(&s.shows))
            .unwrap_or_default();
        let is_active = is_active && focus_state == SearchFocusState::Shows;

        utils::construct_list_widget(&ui.theme, show_items, is_active)
    };

    let (episode_list, n_episodes) = {
        let episode_items = search_results
            .map(|s| search_items(&s.episodes))
            .unwrap_or_default();

        let is_active = is_active && focus_state == SearchFocusState::Episodes;

        utils::construct_list_widget(&ui.theme, episode_items, is_active)
    };

    // 4. Render the page's widgets
    // Render the query input box
    frame.render_widget(
        line_input.widget(is_active && focus_state == SearchFocusState::Input),
        search_input_rect,
    );

    // Render the search result windows.
    // Need mutable access to the list/table states stored inside the page state for rendering.
    let PageState::Search {
        state: page_state, ..
    } = ui.current_page_mut()
    else {
        return;
    };
    utils::render_list_window(
        frame,
        track_list,
        track_rect,
        n_tracks,
        &mut page_state.track_list,
    );
    utils::render_list_window(
        frame,
        album_list,
        album_rect,
        n_albums,
        &mut page_state.album_list,
    );
    utils::render_list_window(
        frame,
        artist_list,
        artist_rect,
        n_artists,
        &mut page_state.artist_list,
    );
    utils::render_list_window(
        frame,
        playlist_list,
        playlist_rect,
        n_playlists,
        &mut page_state.playlist_list,
    );
    utils::render_list_window(
        frame,
        show_list,
        show_rect,
        n_shows,
        &mut page_state.show_list,
    );
    utils::render_list_window(
        frame,
        episode_list,
        episode_rect,
        n_episodes,
        &mut page_state.episode_list,
    );
}

pub fn render_context_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    // 1. Get data
    let PageState::Context {
        id,
        context_page_type,
        ..
    } = ui.current_page()
    else {
        return;
    };

    // 2. Construct the page's layout
    let rect = construct_and_render_block(
        &context_page_type.title(),
        &ui.theme,
        Borders::ALL,
        frame,
        rect,
    );

    // 3+4. Construct and render the page's widgets
    let Some(id) = id else {
        frame.render_widget(
            Paragraph::new("Cannot determine the current page's context"),
            rect,
        );
        return;
    };

    let data = state.data.read();
    match data.caches.context.get(&id.uri()) {
        Some(context) => {
            // render context description
            let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(0)]).split(rect);

            let description = if let Context::Playlist { playlist, .. } = context {
                format!(
                    "{} | {}",
                    context.description(),
                    if data.user_data.is_followed_playlist(playlist) {
                        "Followed"
                    } else {
                        "Not Followed"
                    }
                )
            } else {
                context.description()
            };

            frame.render_widget(
                Paragraph::new(description).style(ui.theme.page_desc()),
                chunks[0],
            );
            let rect = chunks[1];

            match context {
                Context::Artist {
                    top_tracks,
                    albums,
                    related_artists,
                    ..
                } => {
                    render_artist_context_page_windows(
                        is_active,
                        frame,
                        state,
                        ui,
                        &data,
                        rect,
                        (top_tracks, albums, related_artists),
                    );
                }
                Context::Playlist { tracks, playlist } => {
                    let rect = if playlist.desc.is_empty() {
                        rect
                    } else {
                        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(0)])
                            .split(rect);
                        frame.render_widget(
                            Paragraph::new(playlist.desc.clone()).style(ui.theme.playlist_desc()),
                            chunks[0],
                        );
                        chunks[1]
                    };

                    render_track_table(
                        frame,
                        rect,
                        is_active,
                        state,
                        ui.search_filtered_items(tracks),
                        ui,
                        &data,
                    );
                }
                Context::Tracks { tracks, .. } | Context::Album { tracks, .. } => {
                    render_track_table(
                        frame,
                        rect,
                        is_active,
                        state,
                        ui.search_filtered_items(tracks),
                        ui,
                        &data,
                    );
                }
                Context::Show { episodes, .. } => {
                    render_episode_table(
                        frame,
                        rect,
                        is_active,
                        state,
                        ui.search_filtered_items(episodes),
                        ui,
                    );
                }
            }
        }
        None => {
            frame.render_widget(Paragraph::new("Loading..."), rect);
        }
    }
}

pub fn render_library_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    // 1. Get data
    let curr_context_uri = state.player.read().playing_context_id().map(|c| c.uri());
    let data = state.data.read();
    let configs = config::get_config();

    let (focus_state, playlist_folder_id) = match ui.current_page() {
        PageState::Library { state } => (state.focus, state.playlist_folder_id),
        _ => return,
    };

    // 2. Construct the page's layout
    // Split the library page into 3 windows:
    // - a playlists window
    // - a saved albums window
    // - a followed artists window

    let chunks = ui
        .orientation
        .layout([
            Constraint::Percentage(configs.app_config.layout.library.playlist_percent),
            Constraint::Percentage(configs.app_config.layout.library.album_percent),
            Constraint::Percentage(
                100 - (configs.app_config.layout.library.album_percent
                    + configs.app_config.layout.library.playlist_percent),
            ),
        ])
        .split(rect);

    let playlist_rect = construct_and_render_block(
        "Playlists",
        &ui.theme,
        match ui.orientation {
            Orientation::Horizontal => Borders::TOP | Borders::LEFT | Borders::BOTTOM,
            Orientation::Vertical => Borders::ALL,
        },
        frame,
        chunks[0],
    );
    let album_rect = construct_and_render_block(
        "Albums",
        &ui.theme,
        match ui.orientation {
            Orientation::Horizontal => Borders::TOP | Borders::LEFT | Borders::BOTTOM,
            Orientation::Vertical => Borders::ALL,
        },
        frame,
        chunks[1],
    );
    let artist_rect =
        construct_and_render_block("Artists", &ui.theme, Borders::ALL, frame, chunks[2]);

    // 3. Construct the page's widgets
    // Construct the playlist window
    let items = ui
        .search_filtered_items(&data.user_data.folder_playlists_items(playlist_folder_id))
        .into_iter()
        .map(|item| match item {
            PlaylistFolderItem::Playlist(p) => {
                (p.to_bidi_string(), curr_context_uri == Some(p.id.uri()))
            }
            PlaylistFolderItem::Folder(f) => (f.to_bidi_string(), false),
        })
        .collect::<Vec<_>>();

    let (playlist_list, n_playlists) = utils::construct_list_widget(
        &ui.theme,
        items,
        is_active
            && focus_state != LibraryFocusState::SavedAlbums
            && focus_state != LibraryFocusState::FollowedArtists,
    );
    // Construct the saved album window
    let (album_list, n_albums) = utils::construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.saved_albums)
            .into_iter()
            .map(|a| (a.to_bidi_string(), curr_context_uri == Some(a.id.uri())))
            .collect(),
        is_active && focus_state == LibraryFocusState::SavedAlbums,
    );
    // Construct the followed artist window
    let (artist_list, n_artists) = utils::construct_list_widget(
        &ui.theme,
        ui.search_filtered_items(&data.user_data.followed_artists)
            .into_iter()
            .map(|a| (a.to_bidi_string(), curr_context_uri == Some(a.id.uri())))
            .collect(),
        is_active && focus_state == LibraryFocusState::FollowedArtists,
    );

    // 4. Render the page's widgets
    // Render the library page's windows.
    // Will need mutable access to the list/table states stored inside the page state for rendering.
    let PageState::Library { state: page_state } = ui.current_page_mut() else {
        return;
    };

    utils::render_list_window(
        frame,
        playlist_list,
        playlist_rect,
        n_playlists,
        &mut page_state.playlist_list,
    );
    utils::render_list_window(
        frame,
        album_list,
        album_rect,
        n_albums,
        &mut page_state.saved_album_list,
    );
    utils::render_list_window(
        frame,
        artist_list,
        artist_rect,
        n_artists,
        &mut page_state.followed_artist_list,
    );
}

pub fn render_browse_page(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    mut rect: Rect,
) {
    // 1. Get data
    let data = state.data.read();

    // 2+3. Construct the page's layout and widgets
    let (list, len) = match ui.current_page() {
        PageState::Browse { state: ui_state } => match ui_state {
            BrowsePageUIState::CategoryList { .. } => {
                rect =
                    construct_and_render_block("Categories", &ui.theme, Borders::ALL, frame, rect);

                utils::construct_list_widget(
                    &ui.theme,
                    ui.search_filtered_items(&data.browse.categories)
                        .into_iter()
                        .map(|c| (c.name.clone(), false))
                        .collect(),
                    is_active,
                )
            }
            BrowsePageUIState::CategoryPlaylistList { category, .. } => {
                let title = format!("{} Playlists", category.name);
                rect = construct_and_render_block(&title, &ui.theme, Borders::ALL, frame, rect);

                let Some(playlists) = data.browse.category_playlists.get(&category.id) else {
                    frame.render_widget(Paragraph::new("Loading..."), rect);
                    return;
                };

                utils::construct_list_widget(
                    &ui.theme,
                    ui.search_filtered_items(playlists)
                        .into_iter()
                        .map(|c| (c.name.clone(), false))
                        .collect(),
                    is_active,
                )
            }
        },
        _ => return,
    };

    // 4. Render the page's widget
    let Some(MutableWindowState::List(list_state)) = ui.current_page_mut().focus_window_state_mut()
    else {
        return;
    };
    utils::render_list_window(frame, list, rect, len, list_state);
}

pub fn render_lyrics_page(
    _is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    // 1. Get data
    let data = state.data.read();

    // 2. Construct the page's layout
    let rect = construct_and_render_block("Lyrics", &ui.theme, Borders::ALL, frame, rect);
    let chunks = Layout::vertical([Constraint::Length(2), Constraint::Fill(0)]).split(rect);

    // 3. Construct the page's widgets
    let Some(progress) = state.player.read().playback_progress() else {
        frame.render_widget(Paragraph::new("No playback available"), rect);
        return;
    };

    let PageState::Lyrics {
        track_uri,
        track,
        artists,
    } = ui.current_page_mut()
    else {
        return;
    };

    let lyrics = match data.caches.lyrics.get(track_uri) {
        None => {
            frame.render_widget(Paragraph::new("Loading..."), rect);
            return;
        }
        Some(None) => {
            frame.render_widget(Paragraph::new("Lyrics not found"), rect);
            return;
        }
        Some(Some(lyrics)) => lyrics,
    };

    // 4. Render the page's widgets
    // render lyric page description text
    let bidi_track = to_bidi_string(track);
    let bidi_artists = to_bidi_string(artists);
    frame.render_widget(
        Paragraph::new(format!("{bidi_track} by {bidi_artists}")).style(ui.theme.page_desc()),
        chunks[0],
    );

    // render lyric text

    // the last played line id (1-based)
    // zero value indicates no line has been played yet
    let mut last_played_line_id = 0;
    for (id, (t, _)) in lyrics.lines.iter().enumerate() {
        if *t <= progress {
            last_played_line_id = id + 1;
        }
    }
    let lines = lyrics
        .lines
        .iter()
        .enumerate()
        .map(|(id, (_, line))| match (id + 1).cmp(&last_played_line_id) {
            std::cmp::Ordering::Less => Line::styled(line, ui.theme.lyrics_played()),
            std::cmp::Ordering::Equal => Line::styled(line, ui.theme.lyrics_playing()),
            std::cmp::Ordering::Greater => Line::raw(line),
        })
        .collect::<Vec<_>>();

    let mut paragraph = Paragraph::new(lines);
    // keep the currently playing line in the center if
    // the line goes pass the lower half of lyrics section
    let half_height = (chunks[1].height / 2) as usize;
    if let Some(offset) = last_played_line_id.checked_sub(half_height) {
        paragraph = paragraph.scroll((offset as u16, 0));
    }
    frame.render_widget(paragraph, chunks[1]);
}

pub fn render_commands_help_page(frame: &mut Frame, ui: &mut UIStateGuard, rect: Rect) {
    // 1. Get data
    let configs = config::get_config();
    let mut map = BTreeMap::new();
    let keymaps = ui.search_filtered_items(&configs.keymap_config.keymaps);
    keymaps
        .into_iter()
        .filter(|km| km.include_in_help_screen())
        .for_each(|km| {
            let v = map.entry(km.command);
            match v {
                Entry::Vacant(v) => {
                    v.insert(format!("\"{}\"", km.key_sequence));
                }
                Entry::Occupied(mut v) => {
                    let keys = format!("{}, \"{}\"", v.get(), km.key_sequence);
                    *v.get_mut() = keys;
                }
            }
        });

    let scroll_offset = match ui.current_page_mut() {
        PageState::CommandHelp {
            ref mut scroll_offset,
        } => {
            if !map.is_empty() && *scroll_offset >= map.len() {
                *scroll_offset = map.len() - 1;
            }
            *scroll_offset
        }
        _ => return,
    };

    // 2. Construct the page's layout
    let rect = construct_and_render_block("Commands", &ui.theme, Borders::ALL, frame, rect);

    // 3. Construct the page's widget
    let help_table = Table::new(
        map.into_iter()
            .skip(scroll_offset)
            .enumerate()
            .map(|(i, (command, keys))| {
                Row::new(vec![
                    Cell::from(format!("{command:?}")),
                    Cell::from(format!("[{keys}]")),
                    Cell::from(command.desc()),
                ])
                // adding alternating row colors
                .style(if (i + scroll_offset) % 2 == 0 {
                    ui.theme.secondary_row()
                } else {
                    ui.theme.app()
                })
            })
            .collect::<Vec<_>>(),
        COMMAND_TABLE_CONSTRAINTS,
    )
    .header(
        Row::new(vec![
            Cell::from("Command"),
            Cell::from("Shortcuts"),
            Cell::from("Description"),
        ])
        .style(ui.theme.table_header()),
    );

    // 4. Render the page's widget
    frame.render_widget(help_table, rect);
}

pub fn render_queue_page(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    use rspotify::model::{FullEpisode, FullTrack, PlayableItem};
    fn get_playable_name(item: &PlayableItem) -> String {
        match item {
            PlayableItem::Track(FullTrack { ref name, .. })
            | PlayableItem::Episode(FullEpisode { ref name, .. }) => name.clone(),
            PlayableItem::Unknown(_) => String::new(),
        }
    }
    fn get_playable_artists(item: &PlayableItem) -> String {
        match item {
            PlayableItem::Track(FullTrack { ref artists, .. }) => artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            PlayableItem::Episode(FullEpisode { ref show, .. }) => show.publisher.clone(),
            PlayableItem::Unknown(_) => String::new(),
        }
    }
    fn get_playable_duration(item: &PlayableItem) -> String {
        match item {
            PlayableItem::Track(FullTrack { ref duration, .. })
            | PlayableItem::Episode(FullEpisode { ref duration, .. }) => format_duration(duration),
            PlayableItem::Unknown(_) => String::new(),
        }
    }

    // 1. Get data
    let player = state.player.read();
    let queue = match player.queue {
        Some(ref q) => &q.queue,
        None => return,
    };
    let scroll_offset = match ui.current_page_mut() {
        PageState::Queue {
            ref mut scroll_offset,
        } => {
            if !queue.is_empty() && *scroll_offset >= queue.len() {
                *scroll_offset = queue.len() - 1;
            }
            *scroll_offset
        }
        _ => return,
    };

    // 2. Construct the page's layout
    let rect = construct_and_render_block("Queue", &ui.theme, Borders::ALL, frame, rect);

    // 3. Construct the page's widget
    let queue_table = Table::new(
        queue
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .map(|(i, x)| {
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)),
                    Cell::from(get_playable_name(x)),
                    Cell::from(get_playable_artists(x)),
                    Cell::from(get_playable_duration(x)),
                ])
            })
            .collect::<Vec<_>>(),
        [
            Constraint::Percentage(5),
            Constraint::Percentage(40),
            Constraint::Percentage(35),
            Constraint::Percentage(20),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from("#"),
            Cell::from("Title"),
            Cell::from("Artists"),
            Cell::from("Duration"),
        ])
        .style(ui.theme.table_header()),
    );

    // 4. Render page's widget
    frame.render_widget(queue_table, rect);
}

/// Render windows for an artist context page, which includes
/// - A top track table
/// - An album table
/// - A related artist list
fn render_artist_context_page_windows(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    data: &DataReadGuard,
    rect: Rect,
    artist_data: (&[Track], &[Album], &[Artist]),
) {
    // 1. Get data
    let (tracks, albums, artists) = (
        ui.search_filtered_items(artist_data.0),
        ui.search_filtered_items(artist_data.1),
        ui.search_filtered_items(artist_data.2),
    );

    let focus_state = match ui.current_page() {
        PageState::Context {
            state: Some(ContextPageUIState::Artist { focus, .. }),
            ..
        } => *focus,
        _ => return,
    };

    // 2. Construct the page's layout
    // top tracks window
    let chunks = Layout::vertical([Constraint::Length(12), Constraint::Fill(0)]).split(rect);
    let top_tracks_rect = chunks[0];

    // albums and related artitsts windows
    let chunks = Layout::horizontal([Constraint::Ratio(1, 2); 2]).split(chunks[1]);
    let albums_rect = construct_and_render_block(
        "Albums",
        &ui.theme,
        Borders::TOP | Borders::RIGHT,
        frame,
        chunks[0],
    );
    let related_artists_rect =
        construct_and_render_block("Related Artists", &ui.theme, Borders::TOP, frame, chunks[1]);

    // 3. Construct the page's widgets
    // album table

    let is_albums_active = is_active && focus_state == ArtistFocusState::Albums;
    let n_albums = albums.len();
    let album_rows = albums
        .into_iter()
        .map(|a| {
            Row::new(vec![
                Cell::from(a.release_date.clone()),
                Cell::from(a.album_type()),
                Cell::from(a.name.clone()),
            ])
            .style(Style::default())
        })
        .collect::<Vec<_>>();

    let albums_table = Table::new(
        album_rows,
        [
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Fill(1),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from("Date"),
            Cell::from("Type"),
            Cell::from("Name"),
        ])
        .style(ui.theme.table_header()),
    )
    .column_spacing(2)
    .row_highlight_style(ui.theme.selection(is_albums_active));

    // artist list widget
    let (artist_list, n_artists) = {
        let artist_items = artists
            .into_iter()
            .map(|a| (a.name.clone(), false))
            .collect::<Vec<_>>();

        utils::construct_list_widget(
            &ui.theme,
            artist_items,
            is_active && focus_state == ArtistFocusState::RelatedArtists,
        )
    };

    // 4. Render the page's widgets
    render_track_table(
        frame,
        top_tracks_rect,
        is_active && focus_state == ArtistFocusState::TopTracks,
        state,
        tracks,
        ui,
        data,
    );

    let PageState::Context {
        state:
            Some(ContextPageUIState::Artist {
                album_table,
                related_artist_list,
                ..
            }),
        ..
    } = ui.current_page_mut()
    else {
        return;
    };

    utils::render_table_window(frame, albums_table, albums_rect, n_albums, album_table);
    utils::render_list_window(
        frame,
        artist_list,
        related_artists_rect,
        n_artists,
        related_artist_list,
    );
}

fn render_track_table(
    frame: &mut Frame,
    rect: Rect,
    is_active: bool,
    state: &SharedState,
    tracks: Vec<&Track>,
    ui: &mut UIStateGuard,
    data: &DataReadGuard,
) {
    let configs = config::get_config();
    // get the current playing track's URI to decorate such track (if exists) in the track table
    let mut playing_track_uri = String::new();
    let mut playing_id = "";
    if let Some(ref playback) = state.player.read().playback {
        if let Some(rspotify::model::PlayableItem::Track(ref track)) = playback.item {
            playing_track_uri = track
                .id
                .as_ref()
                .map(rspotify::prelude::Id::uri)
                .unwrap_or_default();

            playing_id = if playback.is_playing {
                &configs.app_config.play_icon
            } else {
                &configs.app_config.pause_icon
            };
        }
    }

    // enable Added column if any track in the table has added_at field specified
    let added_at_enabled = tracks.iter().any(|t| t.added_at > 0);

    let n_tracks = tracks.len();
    let rows = tracks
        .into_iter()
        .enumerate()
        .map(|(id, t)| {
            let (id, style) = if playing_track_uri == t.id.uri() {
                (playing_id.to_string(), ui.theme.current_playing())
            } else {
                ((id + 1).to_string(), Style::default())
            };
            Row::new(vec![
                if data.user_data.is_liked_track(t) {
                    Cell::from(&configs.app_config.liked_icon as &str).style(ui.theme.like())
                } else {
                    Cell::from("")
                },
                Cell::from(id),
                Cell::from(to_bidi_string(&t.display_name())),
                Cell::from(to_bidi_string(&t.artists_info())),
                Cell::from(to_bidi_string(&t.album_info())),
                if added_at_enabled {
                    // added_at is in seconds resolution
                    let time =
                        chrono::DateTime::from_timestamp_nanos(t.added_at as i64 * 1_000_000_000);
                    // use absolute date format if the track is added more than a month ago, otherwise use relative date
                    Cell::from(if chrono::Utc::now() > time + chrono::Duration::days(30) {
                        time.format("%b %d, %Y").to_string()
                    } else {
                        HumanTime::from(time).to_string()
                    })
                } else {
                    Cell::from("")
                },
                Cell::from(format!(
                    "{}:{:02}",
                    t.duration.as_secs() / 60,
                    t.duration.as_secs() % 60,
                )),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();
    let track_table = Table::new(
        rows,
        [
            Constraint::Length(configs.app_config.liked_icon.chars().count() as u16),
            Constraint::Length(4),
            Constraint::Fill(4),
            Constraint::Fill(3),
            Constraint::Fill(5),
            if added_at_enabled {
                Constraint::Fill(2)
            } else {
                Constraint::Fill(0)
            },
            Constraint::Fill(1),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(""),
            Cell::from("#"),
            Cell::from("Title"),
            Cell::from("Artists"),
            Cell::from("Album"),
            if added_at_enabled {
                Cell::from("Added")
            } else {
                Cell::from("")
            },
            Cell::from("Duration"),
        ])
        .style(ui.theme.table_header()),
    )
    .column_spacing(2)
    .row_highlight_style(ui.theme.selection(is_active));

    if let PageState::Context {
        state: Some(state), ..
    } = ui.current_page_mut()
    {
        let playable_table_state = match state {
            ContextPageUIState::Artist {
                top_track_table, ..
            } => top_track_table,
            ContextPageUIState::Playlist { track_table }
            | ContextPageUIState::Album { track_table }
            | ContextPageUIState::Tracks { track_table } => track_table,
            ContextPageUIState::Show { .. } => {
                unreachable!("show's episode table should be handled by render_episode_table")
            }
        };
        utils::render_table_window(frame, track_table, rect, n_tracks, playable_table_state);
    }
}

fn render_episode_table(
    frame: &mut Frame,
    rect: Rect,
    is_active: bool,
    state: &SharedState,
    episodes: Vec<&Episode>,
    ui: &mut UIStateGuard,
) {
    let configs = config::get_config();
    // get the current playing episode's URI to decorate such episode (if exists) in the episode table
    let mut playing_episode_uri = String::new();
    let mut playing_id = "";
    if let Some(ref playback) = state.player.read().playback {
        if let Some(rspotify::model::PlayableItem::Episode(ref episode)) = playback.item {
            playing_episode_uri = episode.id.uri();

            playing_id = if playback.is_playing {
                &configs.app_config.play_icon
            } else {
                &configs.app_config.pause_icon
            };
        }
    }

    let n_episodes = episodes.len();
    let rows = episodes
        .into_iter()
        .enumerate()
        .map(|(id, e)| {
            let (id, style) = if playing_episode_uri == e.id.uri() {
                (playing_id.to_string(), ui.theme.current_playing())
            } else {
                ((id + 1).to_string(), Style::default())
            };
            Row::new(vec![
                Cell::from(id),
                Cell::from(to_bidi_string(&e.name)),
                Cell::from(e.release_date.clone()),
                Cell::from(format!(
                    "{}:{:02}",
                    e.duration.as_secs() / 60,
                    e.duration.as_secs() % 60,
                )),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();
    let episode_table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Fill(6),
            Constraint::Fill(2),
            Constraint::Fill(1),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from("#"),
            Cell::from("Title"),
            Cell::from("Date"),
            Cell::from("Duration"),
        ])
        .style(ui.theme.table_header()),
    )
    .column_spacing(2)
    .row_highlight_style(ui.theme.selection(is_active));

    if let PageState::Context {
        state: Some(state), ..
    } = ui.current_page_mut()
    {
        let playable_table_state = match state {
            ContextPageUIState::Show { episode_table } => episode_table,
            s => unreachable!("unexpected state: {s:?}"),
        };
        utils::render_table_window(frame, episode_table, rect, n_episodes, playable_table_state);
    }
}
