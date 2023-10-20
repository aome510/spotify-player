#![allow(unused_imports)]
use souvlaki::MediaPosition;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};

use crate::utils;
use crate::{
    event::{ClientRequest, PlayerRequest},
    state::SharedState,
    utils::map_join,
};

fn update_control_metadata(
    state: &SharedState,
    controls: &mut MediaControls,
    prev_track_info: &mut String,
) -> Result<(), souvlaki::Error> {
    let player = state.player.read();

    match player.current_playing_track() {
        None => {}
        Some(track) => {
            if let Some(ref playback) = player.playback {
                let progress = player
                    .playback_progress()
                    .and_then(|p| Some(MediaPosition(p.to_std().ok()?)));

                if playback.is_playing {
                    controls.set_playback(MediaPlayback::Playing { progress })?;
                } else {
                    controls.set_playback(MediaPlayback::Paused { progress })?;
                }
            }

            // only update metadata when the track information is changed
            let track_info = format!("{}/{}", track.name, track.album.name);
            if track_info != *prev_track_info {
                controls.set_metadata(MediaMetadata {
                    title: Some(&track.name),
                    album: Some(&track.album.name),
                    artist: Some(&map_join(&track.artists, |a| &a.name, ", ")),
                    duration: track.duration.to_std().ok(),
                    cover_url: utils::get_track_album_image_url(track),
                })?;

                *prev_track_info = track_info;
            }
        }
    }

    Ok(())
}

/// Start the application's media control event watcher
pub fn start_event_watcher(
    state: SharedState,
    client_pub: flume::Sender<ClientRequest>,
) -> Result<(), souvlaki::Error> {
    tracing::info!("Initializing application's media control event watcher...");

    #[cfg(not(target_os = "windows"))]
    let hwnd = None;

    #[cfg(target_os = "windows")]
    let (hwnd, _dummy_window) = {
        let dummy_window = windows::DummyWindow::new().unwrap();
        let handle = Some(dummy_window.handle.0 as _);
        (handle, dummy_window)
    };

    let config = PlatformConfig {
        dbus_name: "spotify_player",
        display_name: "Spotify Player",
        hwnd,
    };
    let mut controls = MediaControls::new(config)?;

    controls.attach(move |e| {
        tracing::info!("Got a media control event: {e:?}");
        match e {
            MediaControlEvent::Play => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::Resume))
                    .unwrap_or_default();
            }
            MediaControlEvent::Pause => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::Pause))
                    .unwrap_or_default();
            }
            MediaControlEvent::Toggle => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::ResumePause))
                    .unwrap_or_default();
            }
            MediaControlEvent::SetPosition(MediaPosition(dur)) => {
                if let Ok(dur) = chrono::Duration::from_std(dur) {
                    client_pub
                        .send(ClientRequest::Player(PlayerRequest::SeekTrack(dur)))
                        .unwrap_or_default();
                }
            }
            MediaControlEvent::Next => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::NextTrack))
                    .unwrap_or_default();
            }
            MediaControlEvent::Previous => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::PreviousTrack))
                    .unwrap_or_default();
            }
            _ => {}
        }
    })?;
    // For some reason, on startup, media playback needs to be initialized with `Playing`
    // for the track metadata to be shown up on the MacOS media status bar.
    controls.set_playback(MediaPlayback::Playing { progress: None })?;

    // The below refresh duration should be no less than 1s to avoid **overloading** linux dbus
    // handler provided by the souvlaki library, which only handles an event every 1s.
    // [1]: https://github.com/Sinono3/souvlaki/blob/b4d47bb2797ffdd625c17192df640510466762e1/src/platform/linux/mod.rs#L450
    let refresh_duration = std::time::Duration::from_millis(1000);
    let mut track_info = String::new();
    loop {
        update_control_metadata(&state, &mut controls, &mut track_info)?;
        std::thread::sleep(refresh_duration);

        // this must be run repeatedly to ensure that
        // the Windows event queue is processed by the app
        #[cfg(target_os = "windows")]
        windows::pump_event_queue();
    }
}

// demonstrates how to make a minimal window to allow use of media keys on the command line
// ref: https://github.com/Sinono3/souvlaki/blob/master/examples/print_events.rs
#[cfg(target_os = "windows")]
mod windows {
    use std::io::Error;
    use std::mem;

    use windows::core::PCWSTR;
    use windows::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetAncestor,
        IsDialogMessageW, PeekMessageW, RegisterClassExW, TranslateMessage, GA_ROOT, MSG,
        PM_REMOVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_QUIT, WNDCLASSEXW,
    };

    pub struct DummyWindow {
        pub handle: HWND,
    }

    impl DummyWindow {
        pub fn new() -> Result<DummyWindow, String> {
            let class_name = w!("SimpleTray");

            let handle_result = unsafe {
                let instance = GetModuleHandleW(None)
                    .map_err(|e| (format!("Getting module handle failed: {e}")))?;

                let wnd_class = WNDCLASSEXW {
                    cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
                    hInstance: instance,
                    lpszClassName: class_name,
                    lpfnWndProc: Some(Self::wnd_proc),
                    ..Default::default()
                };

                if RegisterClassExW(&wnd_class) == 0 {
                    return Err(format!(
                        "Registering class failed: {}",
                        Error::last_os_error()
                    ));
                }

                let handle = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class_name,
                    w!(""),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    instance,
                    None,
                );

                if handle.0 == 0 {
                    Err(format!(
                        "Message only window creation failed: {}",
                        Error::last_os_error()
                    ))
                } else {
                    Ok(handle)
                }
            };

            handle_result.map(|handle| DummyWindow { handle })
        }
        extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }

    impl Drop for DummyWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.handle);
            }
        }
    }

    pub fn pump_event_queue() -> bool {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            let mut has_message = PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool();
            while msg.message != WM_QUIT && has_message {
                if !IsDialogMessageW(GetAncestor(msg.hwnd, GA_ROOT), &msg).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                has_message = PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool();
            }

            msg.message == WM_QUIT
        }
    }
}
