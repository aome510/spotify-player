use std::{collections::HashMap, path::Path};

use once_cell::sync::Lazy;
use serde::{de::DeserializeOwned, Serialize};

use super::model::*;

pub type DataReadGuard<'a> = parking_lot::RwLockReadGuard<'a, AppData>;

#[derive(Debug)]
pub enum FileCacheKey {
    Playlists,
    FollowedArtists,
    SavedAlbums,
    SavedTracks,
}

/// default time-to-live cache duration
pub static TTL_CACHE_DURATION: Lazy<std::time::Duration> =
    Lazy::new(|| std::time::Duration::from_secs(60 * 60 * 3));

/// the application's data
pub struct AppData {
    pub user_data: UserData,
    pub caches: MemoryCaches,
    pub browse: BrowseData,
}

#[derive(Debug)]
/// current user's data
pub struct UserData {
    pub user: Option<rspotify_model::PrivateUser>,
    pub playlists: Vec<Playlist>,
    pub followed_artists: Vec<Artist>,
    pub saved_albums: Vec<Album>,
    pub saved_tracks: HashMap<String, Track>,
}

/// the application's in-memory caches
pub struct MemoryCaches {
    pub context: ttl_cache::TtlCache<String, Context>,
    pub search: ttl_cache::TtlCache<String, SearchResults>,
    #[cfg(feature = "lyric-finder")]
    pub lyrics: ttl_cache::TtlCache<String, lyric_finder::LyricResult>,
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
            #[cfg(feature = "lyric-finder")]
            lyrics: ttl_cache::TtlCache::new(64),
            #[cfg(feature = "image")]
            images: ttl_cache::TtlCache::new(64),
        }
    }
}

impl AppData {
    pub fn new(cache_folder: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            user_data: UserData::new_from_file_caches(cache_folder)?,
            caches: MemoryCaches::new(),
            browse: BrowseData::default(),
        })
    }

    pub fn get_tracks_by_id_mut(&mut self, id: &ContextId) -> Option<&mut Vec<Track>> {
        self.caches.context.get_mut(&id.uri()).map(|c| match c {
            Context::Album { tracks, .. } => tracks,
            Context::Playlist { tracks, .. } => tracks,
            Context::Artist {
                top_tracks: tracks, ..
            } => tracks,
            Context::Tracks { tracks, .. } => tracks,
        })
    }
}

impl UserData {
    /// constructs a new user data based on file caches
    pub fn new_from_file_caches(cache_folder: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            user: None,
            playlists: load_data_from_file_cache(FileCacheKey::Playlists, cache_folder)?
                .unwrap_or_default(),
            followed_artists: load_data_from_file_cache(
                FileCacheKey::FollowedArtists,
                cache_folder,
            )?
            .unwrap_or_default(),
            saved_albums: load_data_from_file_cache(FileCacheKey::SavedAlbums, cache_folder)?
                .unwrap_or_default(),
            saved_tracks: load_data_from_file_cache(FileCacheKey::SavedTracks, cache_folder)?
                .unwrap_or_default(),
        })
    }

    /// returns a list of playlists that are **possibly** modifiable by user
    pub fn modifiable_playlists(&self) -> Vec<&Playlist> {
        match self.user {
            None => vec![],
            Some(ref u) => self
                .playlists
                .iter()
                .filter(|p| p.owner.1 == u.id || p.collaborative)
                .collect(),
        }
    }

    /// checks if a track is a liked track
    pub fn is_liked_track(&self, track: &Track) -> bool {
        self.saved_tracks.contains_key(&track.id.uri())
    }
}

pub fn store_data_into_file_cache<T: Serialize>(
    key: FileCacheKey,
    cache_folder: &Path,
    data: &T,
) -> std::io::Result<()> {
    let path = cache_folder.join(format!("{key:?}_cache.json"));
    let f = std::fs::File::create(path)?;
    serde_json::to_writer(f, data)?;
    Ok(())
}

pub fn load_data_from_file_cache<T>(
    key: FileCacheKey,
    cache_folder: &Path,
) -> std::io::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let path = cache_folder.join(format!("{key:?}_cache.json"));
    if path.exists() {
        tracing::info!("Loading {key:?} data from {}...", path.display());
        let f = std::fs::File::open(path)?;
        let data = serde_json::from_reader(f)?;
        tracing::info!("Successfully loaded {key:?} data!");
        Ok(Some(data))
    } else {
        Ok(None)
    }
}
