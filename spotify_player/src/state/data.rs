use std::io::{BufReader, BufWriter};
use std::{collections::HashMap, path::Path};

use serde::{de::DeserializeOwned, Serialize};
use std::sync::LazyLock;

use super::model::{
    Album, Artist, Category, Context, ContextId, Id, Playlist, PlaylistFolderItem,
    PlaylistFolderNode, SearchResults, Show, Track,
};
use super::Lyrics;

pub type DataReadGuard<'a> = parking_lot::RwLockReadGuard<'a, AppData>;

#[derive(Debug, Copy, Clone)]
pub enum FileCacheKey {
    Playlists,
    PlaylistFolders,
    FollowedArtists,
    SavedShows,
    SavedAlbums,
    SavedTracks,
}

/// default time-to-live cache duration
pub static TTL_CACHE_DURATION: LazyLock<std::time::Duration> =
    LazyLock::new(|| std::time::Duration::from_secs(60 * 60));

/// the application's data
pub struct AppData {
    pub user_data: UserData,
    pub caches: MemoryCaches,
    pub browse: BrowseData,
}

#[derive(Debug)]
/// current user's data
pub struct UserData {
    pub user: Option<rspotify::model::PrivateUser>,
    pub playlists: Vec<PlaylistFolderItem>,
    pub playlist_folder_node: Option<PlaylistFolderNode>,
    pub followed_artists: Vec<Artist>,
    pub saved_shows: Vec<Show>,
    pub saved_albums: Vec<Album>,
    pub saved_tracks: HashMap<String, Track>,
}

/// the application's in-memory caches
pub struct MemoryCaches {
    pub context: ttl_cache::TtlCache<String, Context>,
    pub search: ttl_cache::TtlCache<String, SearchResults>,
    pub lyrics: ttl_cache::TtlCache<String, Option<Lyrics>>,
    #[cfg(feature = "image")]
    pub images: ttl_cache::TtlCache<String, image::DynamicImage>,
}

#[derive(Default, Debug)]
/// Spotify browse data
pub struct BrowseData {
    pub categories: Vec<Category>,
    pub category_playlists: HashMap<String, Vec<Playlist>>,
}

impl MemoryCaches {
    pub fn new() -> Self {
        Self {
            context: ttl_cache::TtlCache::new(64),
            search: ttl_cache::TtlCache::new(64),
            lyrics: ttl_cache::TtlCache::new(64),
            #[cfg(feature = "image")]
            images: ttl_cache::TtlCache::new(64),
        }
    }
}

impl AppData {
    pub fn new(cache_folder: &Path) -> Self {
        Self {
            user_data: UserData::new_from_file_caches(cache_folder),
            caches: MemoryCaches::new(),
            browse: BrowseData::default(),
        }
    }

    /// Get a list of tracks inside a given context
    pub fn context_tracks_mut(&mut self, id: &ContextId) -> Option<&mut Vec<Track>> {
        let c = self.caches.context.get_mut(&id.uri())?;

        Some(match c {
            Context::Album { tracks, .. }
            | Context::Playlist { tracks, .. }
            | Context::Tracks { tracks, .. }
            | Context::Artist {
                top_tracks: tracks, ..
            } => tracks,
            Context::Show { .. } => {
                return None;
            }
        })
    }

    pub fn context_tracks(&self, id: &ContextId) -> Option<&Vec<Track>> {
        let c = self.caches.context.get(&id.uri())?;
        Some(match c {
            Context::Album { tracks, .. }
            | Context::Playlist { tracks, .. }
            | Context::Tracks { tracks, .. }
            | Context::Artist {
                top_tracks: tracks, ..
            } => tracks,
            Context::Show { .. } => {
                return None;
            }
        })
    }
}

impl UserData {
    /// Construct a new user data based on file caches
    pub fn new_from_file_caches(cache_folder: &Path) -> Self {
        Self {
            user: None,
            playlists: load_data_from_file_cache(FileCacheKey::Playlists, cache_folder)
                .unwrap_or_default(),
            playlist_folder_node: load_data_from_file_cache(
                FileCacheKey::PlaylistFolders,
                cache_folder,
            ),
            followed_artists: load_data_from_file_cache(
                FileCacheKey::FollowedArtists,
                cache_folder,
            )
            .unwrap_or_default(),
            saved_shows: load_data_from_file_cache(FileCacheKey::SavedShows, cache_folder)
                .unwrap_or_default(),
            saved_albums: load_data_from_file_cache(FileCacheKey::SavedAlbums, cache_folder)
                .unwrap_or_default(),
            saved_tracks: load_data_from_file_cache(FileCacheKey::SavedTracks, cache_folder)
                .unwrap_or_default(),
        }
    }

    /// Get a list of playlist items that are **possibly** modifiable by user
    ///
    /// If `folder_id` is provided, returns items in the given folder id.
    /// Otherwise, returns the all items.
    pub fn modifiable_playlist_items(&self, folder_id: Option<usize>) -> Vec<&PlaylistFolderItem> {
        match self.user {
            None => vec![],
            Some(ref u) => self
                .playlists
                .iter()
                // filter items in a folder (if specified)
                .filter(|item| {
                    if let Some(folder_id) = folder_id {
                        match item {
                            PlaylistFolderItem::Playlist(p) => p.current_folder_id == folder_id,
                            PlaylistFolderItem::Folder(f) => f.current_id == folder_id,
                        }
                    } else {
                        true
                    }
                })
                // filter modifiable items
                .filter(|item| match item {
                    PlaylistFolderItem::Playlist(p) => p.owner.1 == u.id || p.collaborative,
                    PlaylistFolderItem::Folder(_) => true,
                })
                .collect(),
        }
    }

    /// Get playlists items for the given folder id
    pub fn folder_playlists_items(&self, folder_id: usize) -> Vec<&PlaylistFolderItem> {
        self.playlists
            .iter()
            .filter(|item| match item {
                PlaylistFolderItem::Playlist(p) => p.current_folder_id == folder_id,
                PlaylistFolderItem::Folder(f) => f.current_id == folder_id,
            })
            .collect()
    }

    /// Check if a track is a liked track
    pub fn is_liked_track(&self, track: &Track) -> bool {
        self.saved_tracks.contains_key(&track.id.uri())
    }

    /// Check if a playlist is followed
    pub fn is_followed_playlist(&self, playlist: &Playlist) -> bool {
        self.playlists.iter().any(|x| match x {
            PlaylistFolderItem::Playlist(p) => p.id == playlist.id,
            PlaylistFolderItem::Folder(_) => false,
        })
    }
}

pub fn store_data_into_file_cache<T: Serialize>(
    key: FileCacheKey,
    cache_folder: &Path,
    data: &T,
) -> std::io::Result<()> {
    let path = cache_folder.join(format!("{key:?}_cache.json"));
    let f = BufWriter::new(std::fs::File::create(path)?);
    serde_json::to_writer(f, data)?;
    Ok(())
}

pub fn load_data_from_file_cache<T>(key: FileCacheKey, cache_folder: &Path) -> Option<T>
where
    T: DeserializeOwned,
{
    let path = cache_folder.join(format!("{key:?}_cache.json"));
    if path.exists() {
        tracing::info!("Loading {key:?} data from {}...", path.display());
        let f = BufReader::new(std::fs::File::open(path).expect("path exists"));
        match serde_json::from_reader(f) {
            Ok(data) => {
                tracing::info!("Successfully loaded {key:?} data!");
                Some(data)
            }
            Err(err) => {
                tracing::error!("Failed to load {key:?} data: {err:#}");
                None
            }
        }
    } else {
        None
    }
}
