use crate::config;
use librespot::connect::spirc::Spirc;
use librespot::core::{
    config::{ConnectConfig, DeviceType},
    session::Session,
};
use librespot::playback::mixer::MixerConfig;
use librespot::playback::{
    audio_backend,
    config::{AudioFormat, Bitrate, PlayerConfig},
    mixer::{self, Mixer},
    player::Player,
};

#[tokio::main]
/// create a new librespot connection running in the background
pub async fn new_connection(session: Session, device: config::DeviceConfig) {
    // librespot volume is a u16 number ranging from 0 to 65535,
    // while a percentage volume value (from 0 to 100) is used for the device configuration.
    // So we need to convert from one format to another
    let volume = (std::cmp::min(device.volume, 100_u8) as f64 / 100.0 * 65535_f64).round() as u16;

    let connect_config = ConnectConfig {
        name: device.name,
        device_type: device.device_type.parse::<DeviceType>().unwrap_or_default(),
        initial_volume: Some(volume),

        // non-configurable fields, use default values.
        // We may allow users to configure these fields in a future release
        has_volume_ctrl: true,
        autoplay: false,
    };

    log::info!(
        "application's connect configurations: {:#?}",
        connect_config
    );

    let mixer =
        Box::new(mixer::softmixer::SoftMixer::open(MixerConfig::default())) as Box<dyn Mixer>;
    mixer.set_volume(volume);

    let backend = audio_backend::find(None).unwrap();
    let player_config = PlayerConfig {
        bitrate: device
            .bitrate
            .to_string()
            .parse::<Bitrate>()
            .unwrap_or_default(),
        ..Default::default()
    };

    let (player, _channel) = Player::new(
        player_config,
        session.clone(),
        mixer.get_audio_filter(),
        move || backend(None, AudioFormat::default()),
    );

    log::info!("starting an integrated Spotify client using librespot's spirc protocol...");

    let (_spirc, spirc_task) = Spirc::new(connect_config, session, player, mixer);
    spirc_task.await;
}
