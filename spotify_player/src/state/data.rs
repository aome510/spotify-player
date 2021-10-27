use super::model::*;

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
}

#[derive(Debug)]
/// the application's caches
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
