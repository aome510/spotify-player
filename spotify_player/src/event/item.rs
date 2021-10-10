use super::*;

pub fn browse_track_album(
    track: Track,
    send: &mpsc::Sender<ClientRequest>,
    ui: &mut UIStateGuard,
) -> Result<()> {
    if let Some(ref uri) = track.album.uri {
        send.send(ClientRequest::GetContext(ContextURI::Album(uri.clone())))?;
        ui.history.push(PageState::Browsing(uri.clone()));
    }
    Ok(())
}

pub fn browse_track_artist(track: Track, ui: &mut UIStateGuard) {
    let artists = track
        .artists
        .iter()
        .map(|a| Artist {
            name: a.name.clone(),
            uri: a.uri.clone(),
            id: a.id.clone(),
        })
        .filter(|a| a.uri.is_some())
        .collect::<Vec<_>>();
    ui.popup = Some(PopupState::ArtistList(artists, new_list_state()));
}
