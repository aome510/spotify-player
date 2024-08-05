use std::collections::{HashMap, HashSet};

use rspotify::model::Id;

use crate::state::{Playlist, PlaylistFolder, PlaylistFolderItem, PlaylistFolderNode};

/// Structurizes a flat input playlist according to the playlist folder nodes
pub fn structurize(
    playlists: Vec<Playlist>,
    nodes: Vec<PlaylistFolderNode>,
) -> Vec<PlaylistFolderItem> {
    // 1. Collect playlist ids from inner nodes
    let mut playlist_ids: HashSet<String> = HashSet::new();
    get_playlist_ids_from_nodes(&nodes, &mut playlist_ids);
    // 2. Add root playlists that don't belong to folders
    let mut playlist_folders: Vec<PlaylistFolderItem> = Vec::new();
    for playlist in &playlists {
        if !playlist_ids.contains(playlist.id.id()) {
            let mut p = playlist.clone();
            p.current_id = 0;
            playlist_folders.push(PlaylistFolderItem::Playlist(p));
        }
    }
    // 3. Add the rest
    let by_ids: HashMap<String, Playlist> = playlists
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
    folder_id: &mut usize,
    acc: &mut Vec<PlaylistFolderItem>,
) {
    let folder_local_id = *folder_id;
    for f in nodes {
        if let Some((_, id)) = f.uri.rsplit_once(':') {
            if f.node_type == "folder" {
                *folder_id += 1;
                // Folder node
                acc.push(PlaylistFolderItem::Folder(PlaylistFolder {
                    name: f.name.clone().unwrap_or_default(),
                    current_id: folder_local_id,
                    target_id: *folder_id,
                }));
                // Up node
                acc.push(PlaylistFolderItem::Folder(PlaylistFolder {
                    name: format!("← {}", f.name.clone().unwrap_or_default()),
                    current_id: *folder_id,
                    target_id: folder_local_id,
                }));
                add_playlist_folders(&f.children, by_ids, folder_id, acc);
            } else if let Some(playlist) = by_ids.get(id) {
                let mut p = playlist.clone();
                p.current_id = folder_local_id;
                acc.push(PlaylistFolderItem::Playlist(p));
            }
        }
    }
}
