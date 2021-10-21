use std::time;

use crate::command;

use rspotify::model;

pub use rspotify::model::{AlbumId, ArtistId, Id, PlaylistId, TrackId, UserId};

/// Player state
#[derive(Debug)]
pub struct PlayerState {
    pub devices: Vec<Device>,

    pub user: Option<model::PrivateUser>,
    pub user_playlists: Vec<Playlist>,
    pub user_followed_artists: Vec<Artist>,
    pub user_saved_albums: Vec<Album>,

    pub context_id: Option<ContextId>,
    pub context_cache: lru::LruCache<String, Context>,

    pub search_cache: lru::LruCache<String, SearchResults>,

    pub playback: Option<model::CurrentPlaybackContext>,
    pub playback_last_updated: Option<std::time::Instant>,
}

/// Playing context (album, playlist, etc) of the current track
#[derive(Clone, Debug)]
pub enum Context {
    Playlist(Playlist, Vec<Track>),
    Album(Album, Vec<Track>),
    Artist(Artist, Vec<Track>, Vec<Album>, Vec<Artist>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A context Id
pub enum ContextId {
    Playlist(PlaylistId),
    Album(AlbumId),
    Artist(ArtistId),
}

/// A playback which can be either
/// - a Spotify context
/// - a vector of track ids
#[derive(Clone, Debug)]
pub enum Playback {
    Context(ContextId, Option<model::Offset>),
    URIs(Vec<TrackId>, Option<model::Offset>),
}

/// SearchResults denotes the returned data when searching using Spotify API.
/// Search results are returned as pages of Spotify objects, which includes
/// - `tracks`
/// - `albums`
/// - `artists`
/// - `playlists`
#[derive(Default, Clone, Debug)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub playlists: Vec<Playlist>,
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
/// A simplified version of `rspotify` device
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` track
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Option<Album>,
    pub duration: time::Duration,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` album
pub struct Album {
    pub id: AlbumId,
    pub release_date: String,
    pub name: String,
    pub artists: Vec<Artist>,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` artist
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
}

#[derive(Debug, Clone)]
/// A simplified version of `rspotify` playlist
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub owner: (String, UserId),
}

#[derive(Debug, Clone)]
/// A spotify item (track, album, artist, playlist)
pub enum Item {
    Track(Track),
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
}

#[derive(Debug, Clone)]
/// A spotify item as the recommendation seed
pub enum SeedItem {
    Track(Track),
    Artist(Artist),
}

/// A simplified version of `rspotify::CurrentPlaybackContext` containing
/// only fields needed to handle a `event::PlayerRequest`
pub struct SimplifiedPlayback {
    pub device_id: Option<String>,
    pub is_playing: bool,
    pub repeat_state: model::RepeatState,
    pub shuffle_state: bool,
}

impl PlayerState {
    /// gets a simplified playback
    pub fn simplified_playback(&self) -> Option<SimplifiedPlayback> {
        self.playback.as_ref().map(|p| SimplifiedPlayback {
            device_id: p.device.id.clone(),
            is_playing: p.is_playing,
            repeat_state: p.repeat_state,
            shuffle_state: p.shuffle_state,
        })
    }

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
        match self.context_id {
            Some(ref id) => self.context_cache.peek(&id.uri()),
            None => None,
        }
    }

    /// gets the current context (mutable)
    pub fn context_mut(&mut self) -> Option<&mut Context> {
        match self.context_id {
            Some(ref id) => self.context_cache.peek_mut(&id.uri()),
            None => None,
        }
    }
}

impl ContextId {
    pub fn uri(&self) -> String {
        match self {
            Self::Album(ref id) => id.uri(),
            Self::Artist(ref id) => id.uri(),
            Self::Playlist(ref id) => id.uri(),
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
            context_id: None,
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
        self.tracks_mut().sort_by(|x, y| sort_oder.compare(x, y));
    }

    /// reverses order of tracks in the current playing context
    pub fn reverse_tracks(&mut self) {
        self.tracks_mut().reverse();
    }

    /// gets the description of current playing context
    pub fn description(&self) -> String {
        match self {
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
                    playlist.owner.0,
                    tracks.len()
                )
            }
            Context::Artist(ref artist, _, _, _) => {
                format!("Artist: {}", artist.name)
            }
        }
    }

    /// gets all tracks inside the current playing context (immutable)
    pub fn tracks(&self) -> &Vec<Track> {
        match self {
            Context::Album(_, ref tracks) => tracks,
            Context::Playlist(_, ref tracks) => tracks,
            Context::Artist(_, ref tracks, _, _) => tracks,
        }
    }

    /// gets all tracks inside the current playing context (mutable)
    pub fn tracks_mut(&mut self) -> &mut Vec<Track> {
        match self {
            Context::Album(_, ref mut tracks) => tracks,
            Context::Playlist(_, ref mut tracks) => tracks,
            Context::Artist(_, ref mut tracks, _, _) => tracks,
        }
    }
}

impl Device {
    /// tries to convert from a `rspotify_model::Device` into `Device`
    pub fn try_from_device(device: model::Device) -> Option<Self> {
        device.id.as_ref()?;
        Some(Self {
            id: device.id.unwrap(),
            name: device.name,
        })
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
        self.album
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// gets the track information (track's name, artists' name and album's name)
    pub fn track_info(&self) -> String {
        format!(
            "{} {} {}",
            self.name,
            self.artists_info(),
            self.album_info(),
        )
    }

    /// tries to convert from a `rspotify_model::SimplifiedTrack` into `Track`
    pub fn try_from_simplified_track(track: model::SimplifiedTrack) -> Option<Self> {
        track.id.as_ref()?;
        Some(Self {
            id: track.id.unwrap(),
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

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.track_info())
    }
}

impl Album {
    /// tries to convert from a `rspotify_model::SimplifiedAlbum` into `Album`
    pub fn try_from_simplified_album(album: model::SimplifiedAlbum) -> Option<Self> {
        album.id.as_ref()?;
        Some(Self {
            id: album.id.unwrap(),
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

impl std::fmt::Display for Album {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Artist {
    /// tries to convert from a `rspotify_model::SimplifiedArtist` into `Artist`
    pub fn try_from_simplified_artist(artist: model::SimplifiedArtist) -> Option<Self> {
        artist.id.as_ref()?;
        Some(Self {
            id: artist.id.unwrap(),
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

impl std::fmt::Display for Artist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// a helper function to convert a vector of `rspotify_model::SimplifiedArtist` into
/// a vector of `Artist`.
fn from_simplified_artists_to_artists(artists: Vec<model::SimplifiedArtist>) -> Vec<Artist> {
    artists
        .into_iter()
        .map(Artist::try_from_simplified_artist)
        .flatten()
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

impl From<model::FullPlaylist> for Playlist {
    fn from(playlist: model::FullPlaylist) -> Self {
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
                command::Action::BrowseRecommendations,
                command::Action::AddTrackToPlaylist,
                command::Action::SaveToLibrary,
            ],
            Self::Artist(_) => vec![
                command::Action::BrowseRecommendations,
                command::Action::SaveToLibrary,
            ],
            Self::Album(_) => vec![
                command::Action::BrowseArtist,
                command::Action::SaveToLibrary,
            ],
            Self::Playlist(_) => vec![command::Action::SaveToLibrary],
        }
    }
}
