use std::sync::Arc;
use crate::{client::Client, config, state::SharedState};
use librespot_connect::{config::ConnectConfig, spirc::Spirc};
use librespot_core::{config::DeviceType, spotify_id, Session};
use librespot_playback::mixer::MixerConfig;
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, Bitrate, PlayerConfig},
    mixer::{self, Mixer},
    player,
};
use rspotify::model::TrackId;
use serde::Serialize;

#[cfg(not(any(
    feature = "rodio-backend",
    feature = "alsa-backend",
    feature = "pulseaudio-backend",
    feature = "portaudio-backend",
    feature = "jackaudio-backend",
    feature = "rodiojack-backend",
    feature = "sdl-backend",
    feature = "gstreamer-backend"
)))]
compile_error!("Streaming feature is enabled but no audio backend has been selected. Consider adding one of the following features:
    rodio-backend,
    alsa-backend,
    pulseaudio-backend,
    portaudio-backend,
    jackaudio-backend,
    rodiojack-backend,
    sdl-backend,
    gstreamer-backend
For more information, visit https://github.com/aome510/spotify-player?tab=readme-ov-file#streaming
");

#[derive(Debug, Serialize)]
enum PlayerEvent {
    Changed {
        track_id: TrackId<'static>,
    },
    Playing {
        track_id: TrackId<'static>,
        position_ms: u32,
    },
    Paused {
        track_id: TrackId<'static>,
        position_ms: u32,
    },
    EndOfTrack {
        track_id: TrackId<'static>,
    },
}

impl PlayerEvent {
    /// gets the event's arguments
    pub fn args(&self) -> Vec<String> {
        match self {
            PlayerEvent::Changed { track_id } => vec!["Changed".to_string(), track_id.to_string()],
            PlayerEvent::Playing {
                track_id,
                position_ms,
            } => vec![
                "Playing".to_string(),
                track_id.to_string(),
                position_ms.to_string(),
            ],
            PlayerEvent::Paused {
                track_id,
                position_ms,
            } => vec![
                "Paused".to_string(),
                track_id.to_string(),
                position_ms.to_string(),
            ],
            PlayerEvent::EndOfTrack { track_id } => {
                vec!["EndOfTrack".to_string(), track_id.to_string()]
            }
        }
    }
}

fn spotify_id_to_track_id(id: spotify_id::SpotifyId) -> anyhow::Result<TrackId<'static>> {
    let uri = id.to_uri()?;
    Ok(TrackId::from_uri(&uri)?.into_static())
}

impl PlayerEvent {
    pub fn from_librespot_player_event(e: player::PlayerEvent) -> anyhow::Result<Option<Self>> {
        Ok(match e {
            player::PlayerEvent::TrackChanged { audio_item } => Some(PlayerEvent::Changed {
                track_id: spotify_id_to_track_id(audio_item.track_id)?,
            }),
            player::PlayerEvent::Playing {
                track_id,
                position_ms,
                ..
            } => Some(PlayerEvent::Playing {
                track_id: spotify_id_to_track_id(track_id)?,
                position_ms,
            }),
            player::PlayerEvent::Paused {
                track_id,
                position_ms,
                ..
            } => Some(PlayerEvent::Paused {
                track_id: spotify_id_to_track_id(track_id)?,
                position_ms,
            }),
            player::PlayerEvent::EndOfTrack { track_id, .. } => Some(PlayerEvent::EndOfTrack {
                track_id: spotify_id_to_track_id(track_id)?,
            }),
            _ => None,
        })
    }
}

fn execute_player_event_hook_command(
    cmd: &config::Command,
    event: PlayerEvent,
) -> anyhow::Result<()> {
    cmd.execute(Some(event.args()))?;

    Ok(())
}

/// Create a new streaming connection
pub async fn new_connection(client: Client, state: SharedState) -> Spirc {
    let session = client.session().await;
    let configs = config::get_config();
    let device = &configs.app_config.device;

    // `librespot` volume is a u16 number ranging from 0 to 65535,
    // while a percentage volume value (from 0 to 100) is used for the device configuration.
    // So we need to convert from one format to another
    let volume = (std::cmp::min(device.volume, 100_u8) as f64 / 100.0 * 65535.0).round() as u16;

    let connect_config = ConnectConfig {
        name: device.name.clone(),
        device_type: device.device_type.parse::<DeviceType>().unwrap_or_default(),
        initial_volume: Some(volume),

        // non-configurable fields, use default values.
        // We may allow users to configure these fields in a future release
        has_volume_ctrl: true,
        is_group: false,
    };

    tracing::info!("Application's connect configurations: {:?}", connect_config);

    let mixer =
        Arc::new(mixer::softmixer::SoftMixer::open(MixerConfig::default()));
    mixer.set_volume(volume);

    let backend = audio_backend::find(None).expect("should be able to find an audio backend");
    let player_config = PlayerConfig {
        bitrate: device
            .bitrate
            .to_string()
            .parse::<Bitrate>()
            .unwrap_or_default(),
        normalisation: device.normalization,
        ..Default::default()
    };

    tracing::info!(
        "Initializing a new integrated player with device_id={}",
        session.device_id()
    );

    let player = player::Player::new(
        player_config,
        session.clone(),
        mixer.get_soft_volume(),
        move || backend(None, AudioFormat::default()),
    );

    let player1 = player.clone();

    let player_event_task = tokio::task::spawn({
        async move {
            while let Some(event) = player.get_player_event_channel().recv().await {
                match PlayerEvent::from_librespot_player_event(event) {
                    Err(err) => {
                        tracing::warn!("Failed to convert a `librespot` player event into `spotify_player` player event: {err:#}");
                    }
                    Ok(Some(event)) => {
                        tracing::info!("Got a new player event: {event:?}");
                        match event {
                            PlayerEvent::Playing { .. } => {
                                let mut player = state.player.write();
                                if let Some(playback) = player.buffered_playback.as_mut() {
                                    playback.is_playing = true;
                                }
                            }
                            PlayerEvent::Paused { .. } => {
                                let mut player = state.player.write();
                                if let Some(playback) = player.buffered_playback.as_mut() {
                                    playback.is_playing = false;
                                }
                            }
                            _ => {}
                        }
                        client.update_playback(&state);

                        // execute a player event hook command
                        if let Some(ref cmd) = configs.app_config.player_event_hook_command {
                            if let Err(err) = execute_player_event_hook_command(cmd, event) {
                                tracing::warn!(
                                    "Failed to execute player event hook command: {err:#}"
                                );
                            }
                        }
                    }
                    Ok(None) => {}
                }
            }
        }
    });

    tracing::info!("Starting an integrated Spotify player using librespot's spirc protocol");

    // TODO: figure out why needing to create a new session is required
    let new_session = Session::new(session.config().clone(), None);

    let (spirc, spirc_task) = match Spirc::new(
        connect_config,
        new_session,
        session.cache().unwrap().credentials().unwrap(),
        player1,
        mixer,
    )
    .await
    {
        Ok(x) => x,
        Err(e) => panic!("e: {e:?}\nerror: {}\nerror debug: {:?}", e.error, e.error),
    };
    tokio::task::spawn(async move {
        tokio::select! {
            _ = spirc_task => {},
            _ = player_event_task => {}
        }
    });

    spirc
}
