use std::collections::{HashMap, HashSet};

use rspotify::model::{Id, PlaylistId, UserId};

use crate::state::{Playlist, PlaylistFolderNode};

/// Structurizes a flat input playlist according to the playlist folder nodes
pub fn structurize(playlists: &Vec<Playlist>, nodes: Vec<PlaylistFolderNode>) -> Vec<Playlist> {
    // 1. Collect playlist ids from inner nodes
    let mut playlist_ids: HashSet<String> = HashSet::new();
    get_playlist_ids_from_nodes(&nodes, &mut playlist_ids);
    // 2. Add root playlists that don't belong to folders
    let mut playlist_folders: Vec<Playlist> = Vec::new();
    for playlist in playlists {
        if !playlist_ids.contains(playlist.id.id()) {
            let mut p = playlist.clone();
            p.is_folder = false;
            p.level = (0, 0);
            playlist_folders.push(p);
        }
    }
    // 3. Add the rest
    let by_ids: HashMap<String, Playlist> = playlists
        .clone()
        .into_iter()
        .map(|p| (p.id.id().to_string(), p))
        .collect();
    add_playlist_folders(&nodes, &by_ids, &mut 0, &mut playlist_folders);
    playlist_folders
}

fn get_playlist_ids_from_nodes(nodes: &Vec<PlaylistFolderNode>, acc: &mut HashSet<String>) {
    for f in nodes {
        if f.node_type == "folder" {
            get_playlist_ids_from_nodes(&f.children, acc);
        } else {
            acc.insert(f.uri.replace("spotify:playlist:", ""));
        }
    }
}

fn add_playlist_folders(
    nodes: &Vec<PlaylistFolderNode>,
    by_ids: &HashMap<String, Playlist>,
    folder_level: &mut i32,
    acc: &mut Vec<Playlist>,
) {
    let level = *folder_level;
    for f in nodes {
        if let Some((_, id)) = f.uri.rsplit_once(':') {
            if f.node_type == "folder" {
                *folder_level += 1;
                // Folder node
                acc.push(Playlist {
                    id: PlaylistId::from_id("f".to_string() + id)
                        .unwrap()
                        .into_static(),
                    collaborative: false,
                    name: f.name.clone().unwrap_or_default(),
                    owner: ("".to_string(), UserId::from_id(id).unwrap().into_static()),
                    desc: "".to_string(),
                    is_folder: true,
                    level: (level, *folder_level),
                });
                // Up node
                acc.push(Playlist {
                    id: PlaylistId::from_id("u".to_string() + id)
                        .unwrap()
                        .into_static(),
                    collaborative: false,
                    name: "← ".to_string() + f.name.clone().unwrap_or_default().as_str(),
                    owner: ("".to_string(), UserId::from_id(id).unwrap().into_static()),
                    desc: "".to_string(),
                    is_folder: true,
                    level: (*folder_level, level),
                });
                add_playlist_folders(&f.children, by_ids, folder_level, acc);
            } else if let Some(playlist) = by_ids.get(id) {
                let mut p = playlist.clone();
                p.is_folder = false;
                p.level = (level, level);
                acc.push(p);
            }
        }
    }
}
