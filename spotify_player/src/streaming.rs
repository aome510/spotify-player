use std::io::Write;

use crate::{config, event::ClientRequest};
use anyhow::Context;
use librespot_connect::spirc::Spirc;
use librespot_core::{
    config::{ConnectConfig, DeviceType},
    session::Session,
    spotify_id,
};
use librespot_playback::mixer::MixerConfig;
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, Bitrate, PlayerConfig},
    mixer::{self, Mixer},
    player,
};
use rspotify::model::TrackId;
use serde::Serialize;

#[derive(Debug, Serialize)]
enum PlayerEvent {
    Changed {
        old_track_id: TrackId<'static>,
        new_track_id: TrackId<'static>,
    },
    Playing {
        track_id: TrackId<'static>,
        position_ms: u32,
        duration_ms: u32,
    },
    Paused {
        track_id: TrackId<'static>,
        position_ms: u32,
        duration_ms: u32,
    },
    EndOfTrack {
        track_id: TrackId<'static>,
    },
}

fn spotify_id_to_track_id(id: spotify_id::SpotifyId) -> anyhow::Result<TrackId<'static>> {
    let uri = id.to_uri()?;
    Ok(TrackId::from_uri(&uri)?.into_static())
}

impl PlayerEvent {
    pub fn from_librespot_player_event(e: player::PlayerEvent) -> anyhow::Result<Option<Self>> {
        Ok(match e {
            player::PlayerEvent::Changed {
                old_track_id,
                new_track_id,
            } => Some(PlayerEvent::Changed {
                old_track_id: spotify_id_to_track_id(old_track_id)?,
                new_track_id: spotify_id_to_track_id(new_track_id)?,
            }),
            player::PlayerEvent::Playing {
                track_id,
                position_ms,
                duration_ms,
                ..
            } => Some(PlayerEvent::Playing {
                track_id: spotify_id_to_track_id(track_id)?,
                position_ms,
                duration_ms,
            }),
            player::PlayerEvent::Paused {
                track_id,
                position_ms,
                duration_ms,
                ..
            } => Some(PlayerEvent::Paused {
                track_id: spotify_id_to_track_id(track_id)?,
                position_ms,
                duration_ms,
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
    let data = serde_json::to_vec(&event).context("serialize player event into json")?;

    let mut child = std::process::Command::new(&cmd.command)
        .args(&cmd.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => anyhow::bail!("no stdin found in the child command"),
    };

    stdin.write_all(&data)?;
    Ok(())
}

/// Create a new streaming connection
pub fn new_connection(
    session: Session,
    device: config::DeviceConfig,
    client_pub: flume::Sender<ClientRequest>,
    player_event_hook_command: Option<config::Command>,
) -> Spirc {
    // `librespot` volume is a u16 number ranging from 0 to 65535,
    // while a percentage volume value (from 0 to 100) is used for the device configuration.
    // So we need to convert from one format to another
    let volume = (std::cmp::min(device.volume, 100_u8) as f64 / 100.0 * 65535.0).round() as u16;

    let connect_config = ConnectConfig {
        name: device.name,
        device_type: device.device_type.parse::<DeviceType>().unwrap_or_default(),
        initial_volume: Some(volume),

        // non-configurable fields, use default values.
        // We may allow users to configure these fields in a future release
        has_volume_ctrl: true,
        autoplay: false,
    };

    tracing::info!("Application's connect configurations: {:?}", connect_config);

    let mixer =
        Box::new(mixer::softmixer::SoftMixer::open(MixerConfig::default())) as Box<dyn Mixer>;
    mixer.set_volume(volume);

    let backend = audio_backend::find(None).expect("should be able to find an audio backend");
    let player_config = PlayerConfig {
        bitrate: device
            .bitrate
            .to_string()
            .parse::<Bitrate>()
            .unwrap_or_default(),
        ..Default::default()
    };

    tracing::info!(
        "Initializing a new integrated player with device_id={}",
        session.device_id()
    );

    let (player, mut channel) = player::Player::new(
        player_config,
        session.clone(),
        mixer.get_soft_volume(),
        move || backend(None, AudioFormat::default()),
    );

    let player_event_task = tokio::task::spawn({
        async move {
            while let Some(event) = channel.recv().await {
                match PlayerEvent::from_librespot_player_event(event) {
                    Err(err) => {
                        tracing::warn!("Failed to convert a `librespot` player event into `spotify_player` player event: {err:#}");
                    }
                    Ok(Some(event)) => {
                        tracing::info!("Got a new player event: {event:?}");

                        // execute a player event hook command
                        if let Some(ref cmd) = player_event_hook_command {
                            if let Err(err) = execute_player_event_hook_command(cmd, event) {
                                tracing::warn!(
                                    "Failed to execute player event hook command: {err:#}"
                                );
                            }
                        }

                        // notify the application about the new player event by making playback update request
                        client_pub
                            .send_async(ClientRequest::GetCurrentPlayback)
                            .await
                            .unwrap_or_default();
                    }
                    Ok(None) => {}
                }
            }
        }
    });

    tracing::info!("Starting an integrated Spotify player using librespot's spirc protocol");

    let (spirc, spirc_task) = Spirc::new(connect_config, session, player, mixer);
    tokio::task::spawn(async move {
        tokio::select! {
            _ = spirc_task => {},
            _ = player_event_task => {}
        }
    });

    spirc
}
