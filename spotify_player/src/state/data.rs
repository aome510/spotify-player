use super::model::*;

#[derive(Default, Debug)]
pub struct Data {
    pub user: UserData,
    pub devices: Vec<Device>,
    pub caches: Caches,
}

#[derive(Default, Debug)]
pub struct UserData {
    user: Option<rspotify_model::PrivateUser>,
    pub playlists: Vec<Playlist>,
    pub followed_artists: Vec<Artist>,
    pub saved_albums: Vec<Album>,
}

#[derive(Debug)]
pub struct Caches {
    pub context: lru::LruCache<String, Context>,
    pub search: lru::LruCache<String, SearchResults>,
    pub recommendation: lru::LruCache<String, Vec<Track>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            context: lru::LruCache::new(64),
            search: lru::LruCache::new(64),
            recommendation: lru::LruCache::new(64),
        }
    }
}
