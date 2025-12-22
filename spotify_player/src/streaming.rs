use crate::{client::AppClient, config, state::SharedState};
use anyhow::Context;
use librespot_connect::{ConnectConfig, Spirc};
use librespot_core::authentication::Credentials;
use librespot_core::config::DeviceType;
use librespot_core::{spotify_uri, Session, SpotifyUri};
use librespot_playback::mixer::MixerConfig;
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, Bitrate, PlayerConfig},
    mixer::{self, Mixer},
    player,
};
use rspotify::model::{EpisodeId, Id, PlayableId, TrackId};
use serde::Serialize;
use std::sync::Arc;

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
        playable_id: PlayableId<'static>,
    },
    Playing {
        playable_id: PlayableId<'static>,
        position_ms: u32,
    },
    Paused {
        playable_id: PlayableId<'static>,
        position_ms: u32,
    },
    EndOfTrack {
        playable_id: PlayableId<'static>,
    },
}

impl PlayerEvent {
    /// gets the event's arguments
    pub fn args(&self) -> Vec<String> {
        match self {
            PlayerEvent::Changed { playable_id } => {
                vec!["Changed".to_string(), playable_id.uri()]
            }
            PlayerEvent::Playing {
                playable_id,
                position_ms,
            } => vec![
                "Playing".to_string(),
                playable_id.uri(),
                position_ms.to_string(),
            ],
            PlayerEvent::Paused {
                playable_id,
                position_ms,
            } => vec![
                "Paused".to_string(),
                playable_id.uri(),
                position_ms.to_string(),
            ],
            PlayerEvent::EndOfTrack { playable_id } => {
                vec!["EndOfTrack".to_string(), playable_id.uri()]
            }
        }
    }
}

fn spotify_id_to_playable_id(uri: &spotify_uri::SpotifyUri) -> anyhow::Result<PlayableId<'static>> {
    match uri {
        SpotifyUri::Track { .. } => {
            let uri = uri.to_uri()?;
            Ok(TrackId::from_uri(&uri)?.into_static().into())
        }
        SpotifyUri::Episode { .. } => {
            let uri = uri.to_uri()?;
            Ok(EpisodeId::from_uri(&uri)?.into_static().into())
        }
        _ => anyhow::bail!("unexpected spotify_id {uri:?}"),
    }
}

impl PlayerEvent {
    pub fn from_librespot_player_event(e: player::PlayerEvent) -> anyhow::Result<Option<Self>> {
        Ok(match e {
            player::PlayerEvent::TrackChanged { audio_item } => Some(PlayerEvent::Changed {
                playable_id: spotify_id_to_playable_id(&audio_item.track_id)?,
            }),
            player::PlayerEvent::Playing {
                track_id,
                position_ms,
                ..
            } => Some(PlayerEvent::Playing {
                playable_id: spotify_id_to_playable_id(&track_id)?,
                position_ms,
            }),
            player::PlayerEvent::Paused {
                track_id,
                position_ms,
                ..
            } => Some(PlayerEvent::Paused {
                playable_id: spotify_id_to_playable_id(&track_id)?,
                position_ms,
            }),
            player::PlayerEvent::EndOfTrack { track_id, .. } => Some(PlayerEvent::EndOfTrack {
                playable_id: spotify_id_to_playable_id(&track_id)?,
            }),
            _ => None,
        })
    }
}

fn execute_player_event_hook_command(
    cmd: &config::Command,
    event: &PlayerEvent,
) -> anyhow::Result<()> {
    cmd.execute(Some(event.args()))?;

    Ok(())
}

/// Create a new streaming connection
pub async fn new_connection(
    client: AppClient,
    state: SharedState,
    session: Session,
    creds: Credentials,
) -> anyhow::Result<Spirc> {
    let configs = config::get_config();
    let device = &configs.app_config.device;

    // `librespot` volume is a u16 number ranging from 0 to 65535,
    // while a percentage volume value (from 0 to 100) is used for the device configuration.
    // So we need to convert from one format to another
    let volume = (f64::from(std::cmp::min(device.volume, 100_u8)) / 100.0 * 65535.0).round() as u16;

    let connect_config = ConnectConfig {
        name: device.name.clone(),
        device_type: device.device_type.parse::<DeviceType>().unwrap_or_default(),
        initial_volume: volume,

        // non-configurable fields, use default values.
        // We may allow users to configure these fields in a future release
        is_group: false,
        disable_volume: false,
        volume_steps: 64,
    };

    tracing::info!("Application's connect configurations: {:?}", connect_config);

    let mixer = Arc::new(
        mixer::softmixer::SoftMixer::open(MixerConfig::default()).context("opening softmixer")?,
    );
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

    let player_event_task = tokio::task::spawn({
        let mut channel = player.get_player_event_channel();
        async move {
            while let Some(event) = channel.recv().await {
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
                            if let Err(err) = execute_player_event_hook_command(cmd, &event) {
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

    let (spirc, spirc_task) = Spirc::new(connect_config, session, creds, player, mixer)
        .await
        .context("initialize spirc")?;

    tokio::task::spawn(async move {
        tokio::select! {
            () = spirc_task => {},
            _ = player_event_task => {}
        }
    });

    tracing::info!("New streaming connection has been established!");

    Ok(spirc)
}
