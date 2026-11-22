use std::borrow::Cow;

/// formats a time duration into a "{minutes}:{seconds}" format
pub fn format_duration(duration: &chrono::Duration) -> String {
    let secs = duration.num_seconds();
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn map_join<T, F>(v: &[T], f: F, sep: &str) -> String
where
    F: Fn(&T) -> &str,
{
    v.iter().map(f).fold(String::new(), |x, y| {
        if x.is_empty() {
            x + y
        } else {
            x + sep + y
        }
    })
}

#[allow(dead_code)]
pub fn get_track_album_image_url(track: &rspotify::model::FullTrack) -> Option<&str> {
    pick_album_image_url(
        &track.album.images,
        crate::config::get_config()
            .app_config
            .disable_music_video_covers,
    )
}

/// Picks the album image to show. Spotify may list a music video frame before the
/// album cover, so with `prefer_cover` the first image that isn't a video frame wins.
pub fn pick_album_image_url(images: &[rspotify::model::Image], prefer_cover: bool) -> Option<&str> {
    let cover = if prefer_cover {
        get_cover_image_url(images)
    } else {
        None
    };
    cover.or(images.first().map(|i| i.url.as_str()))
}

/// Returns the URL of the first image that isn't a music video frame, if any.
pub fn get_cover_image_url(images: &[rspotify::model::Image]) -> Option<&str> {
    images
        .iter()
        .find(|i| !is_video_frame(i))
        .map(|i| i.url.as_str())
}

/// Whether `images` has images but all of them are music video frames, so a cover has
/// to be borrowed from another release.
pub fn is_music_video_only(images: &[rspotify::model::Image]) -> bool {
    !images.is_empty() && images.iter().all(is_video_frame)
}

/// Whether an image is clearly wider than tall (e.g. a 16:9 music video frame). Album
/// covers are square, and images without dimensions are assumed to be covers.
fn is_video_frame(image: &rspotify::model::Image) -> bool {
    match (image.width, image.height) {
        // wider than 5:4
        (Some(w), Some(h)) => u64::from(w) * 4 > u64::from(h) * 5,
        _ => false,
    }
}

/// Returns the cover URL to display for `track`: the album's own image, or the
/// replacement found for a music-video-only release (see `disable_music_video_covers`).
pub fn get_track_cover_url(
    track: &rspotify::model::FullTrack,
    data: &crate::state::AppData,
) -> Option<String> {
    let url = get_track_album_image_url(track)?;
    Some(
        data.caches
            .cover_urls
            .get(url)
            .cloned()
            .unwrap_or_else(|| url.to_owned()),
    )
}

/// Whether the replacement cover for a music-video-only `track` is still being looked up.
#[allow(dead_code)]
pub fn is_cover_lookup_pending(
    track: &rspotify::model::FullTrack,
    data: &crate::state::AppData,
) -> bool {
    crate::config::get_config()
        .app_config
        .disable_music_video_covers
        && is_music_video_only(&track.album.images)
        && get_track_album_image_url(track)
            .is_some_and(|url| !data.caches.cover_urls.contains_key(url))
}

/// Returns the id of a Spotify image from its URL (`https://i.scdn.co/image/<id>`).
pub fn get_image_id(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or(url)
}

#[allow(dead_code)]
pub fn get_episode_show_image_url(episode: &rspotify::model::FullEpisode) -> Option<&str> {
    if episode.show.images.is_empty() {
        None
    } else {
        Some(&episode.show.images[0].url)
    }
}

pub fn parse_uri(uri: &str) -> Cow<'_, str> {
    let parts = uri.split(':').collect::<Vec<_>>();
    // The below URI probably has a format of `spotify:user:{user_id}:{type}:{id}`,
    // but `rspotify` library expects to receive an URI of format `spotify:{type}:{id}`.
    // We have to modify the URI to a corresponding format.
    // See: https://github.com/aome510/spotify-player/issues/57#issuecomment-1160868626
    if parts.len() == 5 {
        Cow::Owned([parts[0], parts[3], parts[4]].join(":"))
    } else {
        Cow::Borrowed(uri)
    }
}

#[cfg(feature = "fzf")]
use fuzzy_matcher::skim::SkimMatcherV2;

#[cfg(feature = "fzf")]
pub fn fuzzy_search_items<'a, T: std::fmt::Display>(items: &'a [T], query: &str) -> Vec<&'a T> {
    let matcher = SkimMatcherV2::default();
    let mut result = items
        .iter()
        .filter_map(|t| {
            matcher
                .fuzzy(&t.to_string(), query, false)
                .map(|(score, _)| (t, score))
        })
        .collect::<Vec<_>>();

    result.sort_by(|(_, a), (_, b)| b.cmp(a));
    result.into_iter().map(|(t, _)| t).collect::<Vec<_>>()
}

/// Get a list of items filtered by a search query.
pub fn filtered_items_from_query<'a, T: std::fmt::Display>(
    query: &str,
    items: &'a [T],
) -> Vec<&'a T> {
    let query = query.to_lowercase();

    #[cfg(feature = "fzf")]
    return fuzzy_search_items(items, &query);

    #[cfg(not(feature = "fzf"))]
    items
        .iter()
        .filter(|t| {
            if query.is_empty() {
                true
            } else {
                let t = t.to_string().to_lowercase();
                query
                    .split(' ')
                    .filter(|q| !q.is_empty())
                    .all(|q| t.contains(q))
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(url: &str, width: Option<u32>, height: Option<u32>) -> rspotify::model::Image {
        rspotify::model::Image {
            height,
            url: url.to_string(),
            width,
        }
    }

    #[test]
    fn prefers_cover_over_video_frame() {
        let images = [
            image("frame", Some(1920), Some(1080)),
            image("cover", Some(640), Some(640)),
        ];
        assert_eq!(pick_album_image_url(&images, true), Some("cover"));
        assert_eq!(pick_album_image_url(&images, false), Some("frame"));
        assert!(!is_music_video_only(&images));
    }

    #[test]
    fn falls_back_to_first_image_for_music_video_only_release() {
        let images = [
            image("frame1", Some(1920), Some(1080)),
            image("frame2", Some(640), Some(360)),
        ];
        assert_eq!(pick_album_image_url(&images, true), Some("frame1"));
        assert!(is_music_video_only(&images));
    }

    #[test]
    fn images_without_dimensions_and_near_square_images_are_covers() {
        let images = [image("cover", None, None)];
        assert_eq!(get_cover_image_url(&images), Some("cover"));
        assert!(!is_music_video_only(&images));

        let images = [image("cover", Some(640), Some(600))];
        assert_eq!(get_cover_image_url(&images), Some("cover"));
        assert!(!is_music_video_only(&[]));
    }

    #[test]
    fn image_id_is_last_url_segment() {
        assert_eq!(
            get_image_id("https://i.scdn.co/image/ab67616d0000b273abc"),
            "ab67616d0000b273abc"
        );
    }
}
