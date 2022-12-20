pub use rspotify::model as rspotify_model;
pub use rspotify::model::{AlbumId, ArtistId, Id, PlaylistId, TrackId, UserId};

use crate::utils::map_join;

#[derive(Clone, Debug)]
/// A Spotify context (playlist, album, artist)
pub enum Context {
    Playlist {
        playlist: Playlist,
        tracks: Vec<Track>,
    },
    Album {
        album: Album,
        tracks: Vec<Track>,
    },
    Artist {
        artist: Artist,
        top_tracks: Vec<Track>,
        albums: Vec<Album>,
        related_artists: Vec<Artist>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A context Id
pub enum ContextId {
    Playlist(PlaylistId<'static>),
    Album(AlbumId<'static>),
    Artist(ArtistId<'static>),
}

#[derive(Clone, Debug)]
/// Data used to start a new playback.
/// There are two ways to start a new playback:
/// - Specify the playing context ID with an offset
/// - Specify the list of track IDs with an offset
///
/// An offset can be either a track's URI or its absolute offset in the context
pub enum Playback {
    Context(ContextId, Option<rspotify_model::Offset>),
    URIs(Vec<TrackId<'static>>, Option<rspotify_model::Offset>),
}

#[derive(Default, Clone, Debug)]
/// Data returned when searching a query using Spotify APIs.
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub playlists: Vec<Playlist>,
}

#[derive(Debug)]
/// A track order
pub enum TrackOrder {
    AddedAt,
    TrackName,
    Album,
    Artists,
    Duration,
}

#[derive(Debug, Clone)]
/// A Spotify item (track, album, artist, playlist)
pub enum Item {
    Track(Track),
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
}

#[derive(Debug, Clone)]
pub enum ItemId {
    Track(TrackId<'static>),
    Album(AlbumId<'static>),
    Artist(ArtistId<'static>),
    Playlist(PlaylistId<'static>),
}

#[derive(Debug, Clone)]
/// A Spotify recommendation seed
pub enum SeedItem {
    Track(Track),
    Artist(Artist),
}

/// A simplified version of `rspotify::CurrentPlaybackContext`
/// containing only fields needed to handle a `event::PlayerRequest`
pub struct SimplifiedPlayback {
    pub device_id: Option<String>,
    pub is_playing: bool,
    pub repeat_state: rspotify_model::RepeatState,
    pub shuffle_state: bool,
}

#[derive(Debug, Clone)]
/// A Spotify device
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
/// A Spotify track
pub struct Track {
    pub id: TrackId<'static>,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Option<Album>,
    pub duration: std::time::Duration,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
/// A Spotify album
pub struct Album {
    pub id: AlbumId<'static>,
    pub release_date: String,
    pub name: String,
    pub artists: Vec<Artist>,
}

#[derive(Debug, Clone)]
/// A Spotify artist
pub struct Artist {
    pub id: ArtistId<'static>,
    pub name: String,
}

#[derive(Debug, Clone)]
/// A Spotify playlist
pub struct Playlist {
    pub id: PlaylistId<'static>,
    pub collaborative: bool,
    pub name: String,
    pub owner: (String, UserId<'static>),
}

#[derive(Clone, Debug)]
/// A Spotify category
pub struct Category {
    pub id: String,
    pub name: String,
}

impl Context {
    /// sorts tracks in the context by a sort oder
    pub fn sort_tracks(&mut self, sort_order: TrackOrder) {
        self.tracks_mut().sort_by(|x, y| sort_order.compare(x, y));
    }

    /// reverses order of tracks in the context
    pub fn reverse_tracks(&mut self) {
        self.tracks_mut().reverse();
    }

    /// gets the context's description
    pub fn description(&self) -> String {
        match self {
            Context::Album {
                ref album,
                ref tracks,
            } => {
                format!(
                    "Album: {} | {} | {} songs",
                    album.name,
                    album.release_date,
                    tracks.len()
                )
            }
            Context::Playlist {
                ref playlist,
                tracks,
            } => {
                format!(
                    "Playlist: {} | {} | {} songs",
                    playlist.name,
                    playlist.owner.0,
                    tracks.len()
                )
            }
            Context::Artist { ref artist, .. } => {
                format!("Artist: {}", artist.name)
            }
        }
    }

    /// gets context tracks (immutable)
    pub fn tracks(&self) -> &Vec<Track> {
        match self {
            Context::Album { ref tracks, .. } => tracks,
            Context::Playlist { ref tracks, .. } => tracks,
            Context::Artist {
                top_tracks: ref tracks,
                ..
            } => tracks,
        }
    }

    /// gets context tracks (mutable)
    pub fn tracks_mut(&mut self) -> &mut Vec<Track> {
        match self {
            Context::Album { tracks, .. } => tracks,
            Context::Playlist { tracks, .. } => tracks,
            Context::Artist {
                top_tracks: tracks, ..
            } => tracks,
        }
    }
}

impl ContextId {
    pub fn uri(&self) -> String {
        match self {
            Self::Album(id) => id.uri(),
            Self::Artist(id) => id.uri(),
            Self::Playlist(id) => id.uri(),
        }
    }
}

impl TrackOrder {
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

impl SeedItem {
    /// gets the uri of the seed item
    pub fn uri(&self) -> String {
        match self {
            Self::Track(track) => track.id.uri(),
            Self::Artist(artist) => artist.id.uri(),
        }
    }
}

impl Device {
    /// tries to convert from a `rspotify_model::Device` into `Device`
    pub fn try_from_device(device: rspotify_model::Device) -> Option<Self> {
        Some(Self {
            id: device.id?,
            name: device.name,
        })
    }
}

impl Track {
    /// gets the track's artists information
    pub fn artists_info(&self) -> String {
        map_join(&self.artists, |a| &a.name, ", ")
    }

    /// gets the track's album information
    pub fn album_info(&self) -> String {
        self.album
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// tries to convert from a `rspotify_model::SimplifiedTrack` into `Track`
    pub fn try_from_simplified_track(track: rspotify_model::SimplifiedTrack) -> Option<Self> {
        Some(Self {
            id: track.id?,
            name: track.name,
            artists: from_simplified_artists_to_artists(track.artists),
            album: None,
            duration: track.duration,
            added_at: 0,
        })
    }

    /// tries to convert from a `rspotify_model::FullTrack` into `Track`
    pub fn try_from_full_track(track: rspotify_model::FullTrack) -> Option<Self> {
        Some(Self {
            id: track.id?,
            name: track.name,
            artists: from_simplified_artists_to_artists(track.artists),
            album: Album::try_from_simplified_album(track.album),
            duration: track.duration,
            added_at: 0,
        })
    }
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} • {} ▎ {}",
            self.name,
            self.artists_info(),
            self.album_info(),
        )
    }
}

impl Album {
    /// tries to convert from a `rspotify_model::SimplifiedAlbum` into `Album`
    pub fn try_from_simplified_album(album: rspotify_model::SimplifiedAlbum) -> Option<Self> {
        Some(Self {
            id: album.id?,
            name: album.name,
            release_date: album.release_date.unwrap_or_default(),
            artists: from_simplified_artists_to_artists(album.artists),
        })
    }
}

impl From<rspotify_model::FullAlbum> for Album {
    fn from(album: rspotify_model::FullAlbum) -> Self {
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
        write!(
            f,
            "{} • {}",
            self.name,
            map_join(&self.artists, |a| &a.name, ", ")
        )
    }
}

impl Artist {
    /// tries to convert from a `rspotify_model::SimplifiedArtist` into `Artist`
    pub fn try_from_simplified_artist(artist: rspotify_model::SimplifiedArtist) -> Option<Self> {
        Some(Self {
            id: artist.id?,
            name: artist.name,
        })
    }
}

impl From<rspotify_model::FullArtist> for Artist {
    fn from(artist: rspotify_model::FullArtist) -> Self {
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

/// a helper function to convert a vector of `rspotify_model::SimplifiedArtist`
/// into a vector of `Artist`.
fn from_simplified_artists_to_artists(
    artists: Vec<rspotify_model::SimplifiedArtist>,
) -> Vec<Artist> {
    artists
        .into_iter()
        .filter_map(Artist::try_from_simplified_artist)
        .collect()
}

impl From<rspotify_model::SimplifiedPlaylist> for Playlist {
    fn from(playlist: rspotify_model::SimplifiedPlaylist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            collaborative: playlist.collaborative,
            owner: (
                playlist.owner.display_name.unwrap_or_default(),
                playlist.owner.id,
            ),
        }
    }
}

impl From<rspotify_model::FullPlaylist> for Playlist {
    fn from(playlist: rspotify_model::FullPlaylist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            collaborative: playlist.collaborative,
            owner: (
                playlist.owner.display_name.unwrap_or_default(),
                playlist.owner.id,
            ),
        }
    }
}

impl std::fmt::Display for Playlist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} • {}", self.name, self.owner.0)
    }
}

impl From<rspotify_model::category::Category> for Category {
    fn from(c: rspotify_model::category::Category) -> Self {
        Self {
            name: c.name,
            id: c.id,
        }
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
