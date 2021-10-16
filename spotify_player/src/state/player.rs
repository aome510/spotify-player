use std::time;

use crate::command;

use rspotify::model;

/// Player state
#[derive(Debug)]
pub struct PlayerState {
    pub devices: Vec<model::Device>,

    pub user: Option<model::PrivateUser>,
    pub user_playlists: Vec<Playlist>,
    pub user_followed_artists: Vec<Artist>,
    pub user_saved_albums: Vec<Album>,

    pub context_uri: String,
    pub context_cache: lru::LruCache<String, Context>,

    pub search_cache: lru::LruCache<String, SearchResults>,

    pub playback: Option<model::CurrentPlaybackContext>,
    pub playback_last_updated: Option<std::time::Instant>,
}

/// Playing context (album, playlist, etc) of the current track
#[derive(Clone, Debug)]
pub enum Context {
    Playlist(model::FullPlaylist, Vec<Track>),
    Album(model::FullAlbum, Vec<Track>),
    Artist(model::FullArtist, Vec<Track>, Vec<Album>, Vec<Artist>),
    Unknown(String),
}

#[derive(Debug)]
/// A context Id
pub enum ContextId {
    Playlist(model::PlaylistId),
    Album(model::AlbumId),
    Artist(model::ArtistId),
    Unknown(String),
}

/// SearchResults denotes the returned data when searching using Spotify API.
/// Search results are returned as pages of Spotify objects, which includes
/// - `tracks`
/// - `albums`
/// - `artists`
/// - `playlists`
#[derive(Clone, Debug)]
pub struct SearchResults {
    pub tracks: model::Page<Track>,
    pub artists: model::Page<Artist>,
    pub albums: model::Page<Album>,
    pub playlists: model::Page<Playlist>,
}

#[derive(Debug)]
/// Order of sorting tracks in a playing context
pub enum ContextSortOrder {
    AddedAt,
    TrackName,
    Album,
    Artists,
    Duration,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` track
pub struct Track {
    pub id: model::TrackId,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Option<Album>,
    pub duration: time::Duration,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` album
pub struct Album {
    pub id: model::AlbumId,
    pub release_date: String,
    pub name: String,
    pub artists: Vec<Artist>,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` artist
pub struct Artist {
    pub id: model::ArtistId,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: model::PlaylistId,
    pub name: String,
    pub owner: (String, model::UserId),
}

#[derive(Debug, Clone)]
pub enum Item {
    Track(Track),
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
}

impl PlayerState {
    /// gets the current playing track
    pub fn current_playing_track(&self) -> Option<&model::FullTrack> {
        match self.playback {
            None => None,
            Some(ref playback) => match playback.item {
                Some(rspotify::model::PlayableItem::Track(ref track)) => Some(track),
                _ => None,
            },
        }
    }

    /// gets the current playback progress
    pub fn playback_progress(&self) -> Option<time::Duration> {
        match self.playback {
            None => None,
            Some(ref playback) => {
                let progress_ms = playback.progress.unwrap()
                    + if playback.is_playing {
                        std::time::Instant::now()
                            .saturating_duration_since(self.playback_last_updated.unwrap())
                    } else {
                        time::Duration::default()
                    };
                Some(progress_ms)
            }
        }
    }

    /// gets the current context
    pub fn context(&self) -> Option<&Context> {
        self.context_cache.peek(&self.context_uri)
    }

    /// gets the current context (mutable)
    pub fn context_mut(&mut self) -> Option<&mut Context> {
        self.context_cache.peek_mut(&self.context_uri)
    }
}

impl SearchResults {
    fn empty_page<T>() -> model::Page<T> {
        model::Page {
            href: "".to_owned(),
            items: vec![],
            limit: 0,
            next: None,
            offset: 0,
            previous: None,
            total: 0,
        }
    }

    // returns an empty search results
    pub fn empty() -> Self {
        Self {
            tracks: Self::empty_page::<Track>(),
            artists: Self::empty_page::<Artist>(),
            albums: Self::empty_page::<Album>(),
            playlists: Self::empty_page::<Playlist>(),
        }
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            devices: vec![],
            user: None,
            user_playlists: vec![],
            user_saved_albums: vec![],
            user_followed_artists: vec![],
            context_uri: "".to_owned(),
            context_cache: lru::LruCache::new(64),
            search_cache: lru::LruCache::new(64),
            playback: None,
            playback_last_updated: None,
        }
    }
}

impl Context {
    /// sorts tracks in the current playing context given a context sort oder
    pub fn sort_tracks(&mut self, sort_oder: ContextSortOrder) {
        let tracks = self.tracks_mut();
        if let Some(tracks) = tracks {
            tracks.sort_by(|x, y| sort_oder.compare(x, y));
        }
    }

    /// reverses order of tracks in the current playing context
    pub fn reverse_tracks(&mut self) {
        let tracks = self.tracks_mut();
        if let Some(tracks) = tracks {
            tracks.reverse();
        }
    }

    /// gets the description of current playing context
    pub fn description(&self) -> String {
        match self {
            Context::Unknown(_) => {
                "Cannot infer the playing context from the current playback".to_owned()
            }
            Context::Album(ref album, ref tracks) => {
                format!(
                    "Album: {} | {} | {} songs",
                    album.name,
                    album.release_date,
                    tracks.len()
                )
            }
            Context::Playlist(ref playlist, tracks) => {
                format!(
                    "Playlist: {} | {} | {} songs",
                    playlist.name,
                    playlist
                        .owner
                        .display_name
                        .as_ref()
                        .unwrap_or(&"unknown".to_owned()),
                    tracks.len()
                )
            }
            Context::Artist(ref artist, _, _, _) => {
                format!("Artist: {}", artist.name)
            }
        }
    }

    /// gets all tracks inside the current playing context (immutable)
    pub fn tracks(&self) -> Option<&Vec<Track>> {
        match self {
            Context::Unknown(_) => None,
            Context::Album(_, ref tracks) => Some(tracks),
            Context::Playlist(_, ref tracks) => Some(tracks),
            Context::Artist(_, ref tracks, _, _) => Some(tracks),
        }
    }

    /// gets all tracks inside the current playing context (mutable)
    pub fn tracks_mut(&mut self) -> Option<&mut Vec<Track>> {
        match self {
            Context::Unknown(_) => None,
            Context::Album(_, ref mut tracks) => Some(tracks),
            Context::Playlist(_, ref mut tracks) => Some(tracks),
            Context::Artist(_, ref mut tracks, _, _) => Some(tracks),
        }
    }
}

impl Track {
    /// gets the track's artists information
    pub fn artists_info(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// gets the track's album information
    pub fn album_info(&self) -> String {
        self.album.map(|a| a.name).unwrap_or_default()
    }

    /// gets the track basic information (track's name, artists' name and album's name)
    pub fn basic_info(&self) -> String {
        format!(
            "{} {} {}",
            self.name,
            self.artists_info(),
            self.album_info(),
        )
    }

    /// tries to convert from a `rspotify::SimplifiedTrack` into `Track`
    pub fn try_from_simplified_track(track: model::SimplifiedTrack) -> Option<Self> {
        track.id.map(|id| Self {
            id,
            name: track.name,
            artists: from_simplified_artists_to_artists(track.artists),
            album: None,
            duration: track.duration,
            added_at: 0,
        })
    }
}

impl From<model::FullTrack> for Track {
    fn from(track: model::FullTrack) -> Self {
        Self {
            id: track.id,
            name: track.name,
            artists: from_simplified_artists_to_artists(track.artists),
            album: Album::try_from_simplified_album(track.album),
            duration: track.duration,
            added_at: 0,
        }
    }
}

impl Album {
    /// tries to convert from a `rspotify::SimplifiedAlbum` into `Album`
    pub fn try_from_simplified_album(album: model::SimplifiedAlbum) -> Option<Self> {
        album.id.map(|id| Self {
            id,
            name: album.name,
            release_date: album.release_date.unwrap_or_default(),
            artists: from_simplified_artists_to_artists(album.artists),
        })
    }
}

impl From<model::FullAlbum> for Album {
    fn from(album: model::FullAlbum) -> Self {
        Self {
            name: album.name,
            id: album.id,
            release_date: album.release_date,
            artists: from_simplified_artists_to_artists(album.artists),
        }
    }
}

impl Artist {
    /// tries to convert from a `rspotify::SimplifiedArtist` into `Artist`
    pub fn try_from_simplified_artist(artist: model::SimplifiedArtist) -> Option<Self> {
        artist.id.map(|id| Self {
            id,
            name: artist.name,
        })
    }
}

impl From<model::FullArtist> for Artist {
    fn from(artist: model::FullArtist) -> Self {
        Self {
            name: artist.name,
            id: artist.id,
        }
    }
}

/// a helper function to convert a vector of `rspotify_model::SimplifiedArtist` into
/// a vector of `Artist`.
fn from_simplified_artists_to_artists(artists: Vec<model::SimplifiedArtist>) -> Vec<Artist> {
    artists
        .into_iter()
        .map(Artist::try_from_simplified_artist)
        .filter(Option::is_some)
        .map(Option::unwrap)
        .collect()
}

impl From<model::SimplifiedPlaylist> for Playlist {
    fn from(playlist: model::SimplifiedPlaylist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            owner: (
                playlist.owner.display_name.unwrap_or_default(),
                playlist.owner.id,
            ),
        }
    }
}

impl ContextSortOrder {
    pub fn compare(&self, x: &Track, y: &Track) -> std::cmp::Ordering {
        match *self {
            Self::AddedAt => x.added_at.cmp(&y.added_at),
            Self::TrackName => x.name.cmp(&y.name),
            Self::Album => x.album_info().cmp(&y.album_info()),
            Self::Duration => x.duration.cmp(&y.duration),
            Self::Artists => x.artists_info().cmp(&y.artists_info()),
        }
    }
}

impl Item {
    /// gets the list of possible actions on the current item
    pub fn actions(&self) -> Vec<command::Action> {
        match self {
            Self::Track(_) => vec![
                command::Action::BrowseAlbum,
                command::Action::BrowseArtist,
                command::Action::AddTrackToPlaylist,
                command::Action::SaveToLibrary,
            ],
            Self::Artist(_) => vec![command::Action::SaveToLibrary],
            Self::Album(_) => vec![
                command::Action::BrowseArtist,
                command::Action::SaveToLibrary,
            ],
            Self::Playlist(_) => vec![command::Action::SaveToLibrary],
        }
    }
}
