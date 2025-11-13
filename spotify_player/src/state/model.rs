use crate::ui::utils::to_bidi_string;
use crate::utils::map_join;
use html_escape::decode_html_entities;
pub use rspotify::model::{
    AlbumId, ArtistId, EpisodeId, Id, PlayableId, PlaylistId, ShowId, TrackId, UserId,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Display, Write};

/// A trait similar to Display but with bidirectional text support
pub trait BidiDisplay: Display {
    fn to_bidi_string(&self) -> String {
        let disp_str = self.to_string();
        to_bidi_string(&disp_str)
    }
}

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
    Show {
        show: Show,
        episodes: Vec<Episode>,
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
    Show(ShowId<'static>),
}

/// Data used to start a new playback.
/// There are two ways to start a new playback:
/// - Specify the playing context ID with an offset
/// - Specify the list of track IDs with an offset
///
/// An offset can be either a track's URI or its absolute offset in the context
#[derive(Clone, Debug)]
pub enum Playback {
    Context(ContextId, Option<rspotify::model::Offset>),
    URIs(Vec<PlayableId<'static>>, Option<rspotify::model::Offset>),
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
/// Data returned when searching a query using Spotify APIs.
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub playlists: Vec<Playlist>,
    pub shows: Vec<Show>,
    pub episodes: Vec<Episode>,
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
    Show(Show),
}

#[derive(Debug, Clone)]
pub enum ItemId {
    Track(TrackId<'static>),
    Album(AlbumId<'static>),
    Artist(ArtistId<'static>),
    Playlist(PlaylistId<'static>),
    Show(ShowId<'static>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackMetadata {
    pub device_name: String,
    pub device_id: Option<String>,
    pub volume: Option<u32>,
    pub is_playing: bool,
    pub repeat_state: rspotify::model::RepeatState,
    pub shuffle_state: bool,
    pub mute_state: Option<u32>,
    /// Indicate if fake track repeat mode is enabled.
    /// This mode is a workaround for a librespot's [limitation] that doesn't support `track` repeat.
    ///
    /// [limitation]: https://github.com/librespot-org/librespot/issues/19
    pub fake_track_repeat_state: bool,
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
    pub typ: Option<rspotify::model::AlbumType>,
    pub added_at: u64,
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
    pub desc: String,
    /// which folder id the playlist refers to
    #[serde(default)]
    pub current_folder_id: usize,
    pub snapshot_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
/// A Spotify show (podcast)
pub struct Show {
    pub id: ShowId<'static>,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
/// A Spotify episode (podcast episode)
pub struct Episode {
    pub id: EpisodeId<'static>,
    pub name: String,
    pub description: String,
    pub duration: std::time::Duration,
    pub show: Option<Show>,
    pub release_date: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
/// A playlist folder, not related to Spotify API yet
pub struct PlaylistFolder {
    pub name: String,
    /// current folder id in the folders tree
    pub current_id: usize,
    /// target folder id it refers to
    pub target_id: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// A playlist folder item
pub enum PlaylistFolderItem {
    Playlist(Playlist),
    Folder(PlaylistFolder),
}

#[derive(Deserialize, Debug, Clone)]
/// A reference node retrieved by running <https://github.com/mikez/spotify-folders>
/// Helps building a playlist folder hierarchy
pub struct PlaylistFolderNode {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub uri: String,
    #[serde(default = "Vec::new")]
    pub children: Vec<PlaylistFolderNode>,
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
            } => format!(
                "{} | {} | {} songs | {}",
                album.name,
                album.release_date,
                tracks.len(),
                play_time(tracks),
            ),
            Context::Playlist {
                ref playlist,
                tracks,
            } => format!(
                "{} | {} | {} songs | {}",
                playlist.name,
                playlist.owner.0,
                tracks.len(),
                play_time(tracks),
            ),
            Context::Artist { ref artist, .. } => artist.name.clone(),
            Context::Tracks { desc, tracks } => {
                format!("{} | {} songs | {}", desc, tracks.len(), play_time(tracks))
            }
            Context::Show {
                ref show,
                ref episodes,
            } => format!("{} | {} episodes", show.name, episodes.len()),
        }
    }
}

fn play_time(tracks: &[Track]) -> String {
    let duration = tracks
        .iter()
        .map(|t| t.duration)
        .sum::<std::time::Duration>();

    let mut output = String::new();

    let seconds = duration.as_secs() % 60;
    let minutes = (duration.as_secs() / 60) % 60;
    let hours = duration.as_secs() / 3600;

    if hours > 0 {
        write!(output, "{hours}h ").unwrap();
    }

    if minutes > 0 {
        write!(output, "{minutes}m ").unwrap();
    }

    write!(output, "{seconds}s").unwrap();

    output
}

impl ContextId {
    pub fn uri(&self) -> String {
        match self {
            Self::Album(id) => id.uri(),
            Self::Artist(id) => id.uri(),
            Self::Playlist(id) => id.uri(),
            Self::Tracks(id) => id.uri.clone(),
            Self::Show(id) => id.uri(),
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
    /// tries to convert from a `rspotify::model::Device` into `Device`
    pub fn try_from_device(device: rspotify::model::Device) -> Option<Self> {
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

    /// tries to convert from a `rspotify::model::SimplifiedTrack` into `Track`
    pub fn try_from_simplified_track(track: rspotify::model::SimplifiedTrack) -> Option<Self> {
        if track.is_playable.unwrap_or(true) {
            let id = match track.linked_from {
                Some(d) => d.id?,
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

    /// tries to convert from a `rspotify::model::FullTrack` into `Track` with a optional `added_at` date
    fn try_from_full_track_with_date(
        track: rspotify::model::FullTrack,
        added_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Option<Self> {
        if track.is_playable.unwrap_or(true) {
            let id = match track.linked_from {
                Some(d) => d.id?,
                None => track.id?,
            };
            Some(Self {
                id,
                name: track.name,
                artists: from_simplified_artists_to_artists(track.artists),
                album: Album::try_from_simplified_album(track.album),
                duration: track.duration.to_std().expect("valid chrono duration"),
                explicit: track.explicit,
                added_at: added_at.map(|t| t.timestamp() as u64).unwrap_or_default(),
            })
        } else {
            None
        }
    }

    /// tries to convert from a `rspotify::model::FullTrack` into `Track`
    pub fn try_from_full_track(track: rspotify::model::FullTrack) -> Option<Self> {
        Track::try_from_full_track_with_date(track, None)
    }

    /// tries to convert from a `rspotify::model::PlaylistItem` into `Track`
    pub fn try_from_playlist_item(item: rspotify::model::PlaylistItem) -> Option<Self> {
        let rspotify::model::PlayableItem::Track(track) = item.track? else {
            return None;
        };

        Track::try_from_full_track_with_date(track, item.added_at)
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

impl BidiDisplay for Track {}

impl Album {
    /// tries to convert from a `rspotify::model::SimplifiedAlbum` into `Album`
    pub fn try_from_simplified_album(album: rspotify::model::SimplifiedAlbum) -> Option<Self> {
        Some(Self {
            id: album.id?,
            name: album.name,
            release_date: album.release_date.unwrap_or_default(),
            artists: from_simplified_artists_to_artists(album.artists),
            typ: album
                .album_type
                .and_then(|t| match t.to_ascii_lowercase().as_str() {
                    "album" => Some(rspotify::model::AlbumType::Album),
                    "single" => Some(rspotify::model::AlbumType::Single),
                    "appears_on" => Some(rspotify::model::AlbumType::AppearsOn),
                    "compilation" => Some(rspotify::model::AlbumType::Compilation),
                    _ => None,
                }),
            added_at: 0,
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

    /// gets the album type
    pub fn album_type(&self) -> String {
        match self.typ {
            Some(t) => <&str>::from(t).to_string(),
            _ => String::new(),
        }
    }
}

impl From<rspotify::model::FullAlbum> for Album {
    fn from(album: rspotify::model::FullAlbum) -> Self {
        Self {
            name: album.name,
            id: album.id,
            release_date: album.release_date,
            artists: from_simplified_artists_to_artists(album.artists),
            typ: Some(album.album_type),
            added_at: 0,
        }
    }
}

impl From<rspotify::model::SavedAlbum> for Album {
    fn from(saved_album: rspotify::model::SavedAlbum) -> Self {
        let mut album: Album = saved_album.album.into();
        album.added_at = saved_album.added_at.timestamp() as u64;
        album
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

impl BidiDisplay for Album {}

impl Artist {
    /// tries to convert from a `rspotify::model::SimplifiedArtist` into `Artist`
    pub fn try_from_simplified_artist(artist: rspotify::model::SimplifiedArtist) -> Option<Self> {
        Some(Self {
            id: artist.id?,
            name: artist.name,
        })
    }
}

impl From<rspotify::model::FullArtist> for Artist {
    fn from(artist: rspotify::model::FullArtist) -> Self {
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

/// a helper function to convert a vector of `rspotify::model::SimplifiedArtist`
/// into a vector of `Artist`.
fn from_simplified_artists_to_artists(
    artists: Vec<rspotify::model::SimplifiedArtist>,
) -> Vec<Artist> {
    artists
        .into_iter()
        .filter_map(Artist::try_from_simplified_artist)
        .collect()
}

impl BidiDisplay for Artist {}

impl From<rspotify::model::SimplifiedPlaylist> for Playlist {
    fn from(playlist: rspotify::model::SimplifiedPlaylist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name,
            collaborative: playlist.collaborative,
            owner: (
                playlist.owner.display_name.unwrap_or_default(),
                playlist.owner.id,
            ),
            desc: String::new(),
            current_folder_id: 0,
            snapshot_id: playlist.snapshot_id,
        }
    }
}

impl From<rspotify::model::FullPlaylist> for Playlist {
    fn from(playlist: rspotify::model::FullPlaylist) -> Self {
        // remove HTML tags from the description
        let re = regex::Regex::new("(<.*?>|</.*?>)").expect("valid regex");
        let desc = playlist.description.unwrap_or_default();
        let desc = decode_html_entities(&re.replace_all(&desc, "")).to_string();

        Self {
            id: playlist.id,
            name: playlist.name,
            collaborative: playlist.collaborative,
            owner: (
                playlist.owner.display_name.unwrap_or_default(),
                playlist.owner.id,
            ),
            desc,
            current_folder_id: 0,
            snapshot_id: playlist.snapshot_id,
        }
    }
}

impl std::fmt::Display for Playlist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} • {}", self.name, self.owner.0)
    }
}

impl BidiDisplay for Playlist {}

impl From<rspotify::model::SimplifiedShow> for Show {
    fn from(show: rspotify::model::SimplifiedShow) -> Self {
        Self {
            id: show.id,
            name: show.name,
        }
    }
}

impl From<rspotify::model::FullShow> for Show {
    fn from(show: rspotify::model::FullShow) -> Self {
        Self {
            id: show.id,
            name: show.name,
        }
    }
}

impl std::fmt::Display for Show {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl BidiDisplay for Show {}

impl From<rspotify::model::SimplifiedEpisode> for Episode {
    fn from(episode: rspotify::model::SimplifiedEpisode) -> Self {
        Self {
            id: episode.id,
            name: episode.name,
            description: episode.description,
            duration: episode.duration.to_std().expect("valid chrono duration"),
            show: None,
            release_date: episode.release_date,
        }
    }
}

impl From<rspotify::model::FullEpisode> for Episode {
    fn from(episode: rspotify::model::FullEpisode) -> Self {
        Self {
            id: episode.id,
            name: episode.name,
            description: episode.description,
            duration: episode.duration.to_std().expect("valid chrono duration"),
            show: Some(episode.show.into()),
            release_date: episode.release_date,
        }
    }
}

impl std::fmt::Display for Episode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.show {
            write!(f, "{} • {}", self.name, s.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl std::fmt::Display for PlaylistFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/", self.name)
    }
}

impl BidiDisplay for PlaylistFolder {}

impl std::fmt::Display for PlaylistFolderItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaylistFolderItem::Playlist(playlist) => playlist.fmt(f),
            PlaylistFolderItem::Folder(folder) => folder.fmt(f),
        }
    }
}

impl BidiDisplay for PlaylistFolderItem {}

impl From<rspotify::model::category::Category> for Category {
    fn from(c: rspotify::model::category::Category) -> Self {
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
                Playback::Context(id.clone(), Some(rspotify::model::Offset::Uri(uri)))
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

                Playback::URIs(ids, Some(rspotify::model::Offset::Uri(uri)))
            }
        }
    }
}

impl PlaybackMetadata {
    pub fn from_playback(p: &rspotify::model::CurrentPlaybackContext) -> Self {
        Self {
            device_name: p.device.name.clone(),
            device_id: p.device.id.clone(),
            is_playing: p.is_playing,
            volume: p.device.volume_percent,
            repeat_state: p.repeat_state,
            shuffle_state: p.shuffle_state,
            mute_state: None,
            fake_track_repeat_state: false,
        }
    }
}

#[derive(Debug)]
pub struct Lyrics {
    /// Timestamped lines
    pub lines: Vec<(chrono::Duration, String)>,
}

impl From<librespot_metadata::lyrics::Lyrics> for Lyrics {
    fn from(value: librespot_metadata::lyrics::Lyrics) -> Self {
        let mut lines = value
            .lyrics
            .lines
            .into_iter()
            .map(|l| {
                let t = chrono::Duration::milliseconds(
                    l.start_time_ms.parse::<i64>().expect("invalid number"),
                );

                (t, to_bidi_string(&l.words))
            })
            .collect::<Vec<_>>();
        lines.sort_by_key(|l| l.0);
        Self { lines }
    }
}
