use super::model::*;

pub type DataReadGuard<'a> = parking_lot::RwLockReadGuard<'a, AppData>;

#[derive(Default, Debug)]
/// the application's data
pub struct AppData {
    pub user_data: UserData,
    pub caches: Caches,
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

#[derive(Debug)]
/// the application's caches
pub struct Caches {
    pub context: lru::LruCache<String, Context>,
    pub search: lru::LruCache<String, SearchResults>,
    pub tracks: lru::LruCache<String, Vec<Track>>,
    #[cfg(feature = "lyric-finder")]
    pub lyrics: lru::LruCache<String, lyric_finder::LyricResult>,
    #[cfg(feature = "image")]
    pub images: lru::LruCache<String, image::DynamicImage>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            context: lru::LruCache::new(64),
            search: lru::LruCache::new(64),
            tracks: lru::LruCache::new(64),
            #[cfg(feature = "lyric-finder")]
            lyrics: lru::LruCache::new(64),
            #[cfg(feature = "image")]
            images: lru::LruCache::new(64),
        }
    }
}

impl UserData {
    /// returns a list of playlists created by the current user
    pub fn playlists_created_by_user(&self) -> Vec<&Playlist> {
        match self.user {
            None => vec![],
            Some(ref u) => self
                .playlists
                .iter()
                .filter(|p| p.owner.1 == u.id)
                .collect(),
        }
    }
}
