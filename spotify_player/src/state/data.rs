use super::model::*;

pub struct Data {
    pub user: UserData,
    pub devices: Vec<Device>,
    pub caches: Caches,
}

pub struct UserData {
    user: Option<rspotify_model::PrivateUser>,
    pub playlists: Vec<Playlist>,
    pub followed_artists: Vec<Artist>,
    pub saved_albums: Vec<Album>,
}

pub struct Caches {
    pub context: lru::LruCache<String, Context>,
    pub search: lru::LruCache<String, SearchResults>,
}
