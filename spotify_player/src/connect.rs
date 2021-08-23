use crate::config;
use librespot_connect::spirc::Spirc;
use librespot_core::{
    config::{ConnectConfig, DeviceType, VolumeCtrl},
    session::Session,
};
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, PlayerConfig},
    mixer::{self, Mixer},
    player::Player,
};

#[tokio::main]
/// create a new librespot connect running in background
pub async fn new_connection(session: Session, device: config::DeviceConfig) {
    let connect_config = ConnectConfig {
        name: device.name,
        device_type: device.device_type.parse::<DeviceType>().unwrap_or_default(),
        volume: device.volume,
        volume_ctrl: device.volume_ctrl.parse::<VolumeCtrl>().unwrap_or_default(),
        autoplay: device.autoplay,
    };
    let backend = audio_backend::find(None).unwrap();
    let player_config = PlayerConfig::default();
    let (player, _channel) = Player::new(player_config, session.clone(), None, move || {
        backend(None, AudioFormat::default())
    });
    let mixer = Box::new(mixer::softmixer::SoftMixer::open(None)) as Box<dyn Mixer>;
    let (_spirc, spirc_task) = Spirc::new(connect_config, session, player, mixer);
    spirc_task.await;
}
