use std::collections::HashMap;

use once_cell::sync::Lazy;

use super::model::*;

pub type DataReadGuard<'a> = parking_lot::RwLockReadGuard<'a, AppData>;

// cache duration, which is default to be 3h
pub static CACHE_DURATION: Lazy<std::time::Duration> =
    Lazy::new(|| std::time::Duration::from_secs(60 * 60 * 3));

#[derive(Default)]
/// the application's data
pub struct AppData {
    pub user_data: UserData,
    pub caches: Caches,
    pub browse: BrowseData,
}

#[derive(Default, Debug)]
/// current user's data
pub struct UserData {
    pub user: Option<rspotify_model::PrivateUser>,
    pub playlists: Vec<Playlist>,
    pub followed_artists: Vec<Artist>,
    pub saved_albums: Vec<Album>,
    pub saved_tracks: Vec<Track>,
}

/// the application's caches
pub struct Caches {
    pub context: ttl_cache::TtlCache<String, Context>,
    pub search: ttl_cache::TtlCache<String, SearchResults>,
    #[cfg(feature = "lyric-finder")]
    pub lyrics: ttl_cache::TtlCache<String, lyric_finder::LyricResult>,
    #[cfg(feature = "image")]
    pub images: ttl_cache::TtlCache<String, image::DynamicImage>,
}

#[derive(Default, Debug)]
pub struct BrowseData {
    pub categories: Vec<Category>,
    pub category_playlists: HashMap<String, Vec<Playlist>>,
}

impl Default for Caches {
    fn default() -> Self {
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
        self.saved_tracks.iter().any(|t| t.id == track.id)
    }
}
