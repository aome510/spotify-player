pub use rspotify::model as rspotify_model;
use rspotify::model::CurrentPlaybackContext;
pub use rspotify::model::{AlbumId, ArtistId, Id, PlaylistId, TrackId, UserId};

use crate::utils::map_join;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
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
    Tracks {
        tracks: Vec<Track>,
        desc: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracksId {
    pub uri: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// A context Id
pub enum ContextId {
    Playlist(PlaylistId<'static>),
    Album(AlbumId<'static>),
    Artist(ArtistId<'static>),
    Tracks(TracksId),
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

/// A simplified version of `rspotify::CurrentPlaybackContext`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimplifiedPlayback {
    pub device_name: String,
    pub device_id: Option<String>,
    pub volume: Option<u32>,
    pub is_playing: bool,
    pub repeat_state: rspotify_model::RepeatState,
    pub shuffle_state: bool,
    pub mute_state: Option<u32>,
}

#[derive(Debug, Clone)]
/// A Spotify device
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
/// A Spotify track
pub struct Track {
    pub id: TrackId<'static>,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Option<Album>,
    pub duration: std::time::Duration,
    pub explicit: bool,
    #[serde(skip)]
    pub added_at: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
/// A Spotify album
pub struct Album {
    pub id: AlbumId<'static>,
    pub release_date: String,
    pub name: String,
    pub artists: Vec<Artist>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
/// A Spotify artist
pub struct Artist {
    pub id: ArtistId<'static>,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
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
    /// gets the context's description
    pub fn description(&self) -> String {
        match self {
            Context::Album {
                ref album,
                ref tracks,
            } => {
                format!(
                    "{} | {} | {} songs",
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
                    "{} | {} | {} songs",
                    playlist.name,
                    playlist.owner.0,
                    tracks.len()
                )
            }
            Context::Artist { ref artist, .. } => artist.name.to_string(),
            Context::Tracks { desc, tracks } => format!("{} | {} songs", desc, tracks.len()),
        }
    }
}

impl ContextId {
    pub fn uri(&self) -> String {
        match self {
            Self::Album(id) => id.uri(),
            Self::Artist(id) => id.uri(),
            Self::Playlist(id) => id.uri(),
            Self::Tracks(id) => id.uri.to_owned(),
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

    /// gets the track's name, including an explicit label
    pub fn display_name(&self) -> Cow<'_, str> {
        if self.explicit {
            Cow::Owned(format!("{} (E)", self.name))
        } else {
            Cow::Borrowed(self.name.as_str())
        }
    }

    /// tries to convert from a `rspotify_model::SimplifiedTrack` into `Track`
    pub fn try_from_simplified_track(track: rspotify_model::SimplifiedTrack) -> Option<Self> {
        if track.is_playable.unwrap_or(true) {
            let id = match track.linked_from {
                Some(d) => d.id,
                None => track.id?,
            };
            Some(Self {
                id,
                name: track.name,
                artists: from_simplified_artists_to_artists(track.artists),
                album: None,
                duration: track.duration.to_std().expect("valid chrono duration"),
                explicit: track.explicit,
                added_at: 0,
            })
        } else {
            None
        }
    }

    /// tries to convert from a `rspotify_model::FullTrack` into `Track`
    pub fn try_from_full_track(track: rspotify_model::FullTrack) -> Option<Self> {
        if track.is_playable.unwrap_or(true) {
            let id = match track.linked_from {
                Some(d) => d.id,
                None => track.id?,
            };
            Some(Self {
                id,
                name: track.name,
                artists: from_simplified_artists_to_artists(track.artists),
                album: Album::try_from_simplified_album(track.album),
                duration: track.duration.to_std().expect("valid chrono duration"),
                explicit: track.explicit,
                added_at: 0,
            })
        } else {
            None
        }
    }
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} • {} ▎ {}",
            self.display_name(),
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

    /// gets the album's release year
    pub fn year(&self) -> String {
        self.release_date
            .split('-')
            .next()
            .unwrap_or("")
            .to_string()
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
            "{} • {} ({})",
            self.name,
            map_join(&self.artists, |a| &a.name, ", "),
            self.year()
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

impl TracksId {
    pub fn new<U, K>(uri: U, kind: K) -> Self
    where
        U: Into<String>,
        K: Into<String>,
    {
        Self {
            uri: uri.into(),
            kind: kind.into(),
        }
    }
}

impl Playback {
    /// creates new playback with a specified offset based on the current playback
    pub fn uri_offset(&self, uri: String, limit: usize) -> Self {
        match self {
            Playback::Context(id, _) => {
                Playback::Context(id.clone(), Some(rspotify_model::Offset::Uri(uri)))
            }
            Playback::URIs(ids, _) => {
                let ids = if ids.len() < limit {
                    ids.clone()
                } else {
                    let pos = ids
                        .iter()
                        .position(|id| id.uri() == uri)
                        .unwrap_or_default();
                    let l = pos.saturating_sub(limit / 2);
                    let r = std::cmp::min(l + limit, ids.len());
                    // For a list with too many tracks, to avoid payload limit when making the `start_playback`
                    // API request, we restrict the range of tracks to be played, which is based on the
                    // playing track's position (if any) and the application's limit (`app_config.tracks_playback_limit`).
                    // Related issue: https://github.com/aome510/spotify-player/issues/78
                    ids[l..r].to_vec()
                };

                Playback::URIs(ids, Some(rspotify_model::Offset::Uri(uri)))
            }
        }
    }
}

impl SimplifiedPlayback {
    pub fn from_playback(p: &CurrentPlaybackContext) -> Self {
        Self {
            device_name: p.device.name.clone(),
            device_id: p.device.id.clone(),
            is_playing: p.is_playing,
            volume: p.device.volume_percent,
            repeat_state: p.repeat_state,
            shuffle_state: p.shuffle_state,
            mute_state: None,
        }
    }
}
