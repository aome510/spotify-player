//! Persistence for the last known playback volume.
//!
//! The integrated `librespot` device is created with a fixed `initial_volume`, so every
//! reconnection (a new session after the network drops, or simply restarting the app) used to
//! reset playback to `device.volume` from the config. Remembering the last volume the user
//! actually set and replaying it on connect keeps the level stable across those events.

use crate::config;

/// File (inside the cache folder) holding the last volume, as a plain percentage.
const VOLUME_STATE_FILE: &str = "volume.state";

fn state_file_path() -> std::path::PathBuf {
    config::get_config().cache_folder.join(VOLUME_STATE_FILE)
}

/// Reads the last persisted volume, or `None` if it was never stored or is unreadable.
// Only read back when creating an integrated player, which is gated behind `streaming`.
#[cfg_attr(not(feature = "streaming"), allow(dead_code))]
pub fn load() -> Option<u8> {
    let path = state_file_path();
    let raw = std::fs::read_to_string(&path).ok()?;
    match raw.trim().parse::<u8>() {
        Ok(volume) if volume <= 100 => Some(volume),
        _ => {
            tracing::warn!("Ignoring malformed persisted volume in {}", path.display());
            None
        }
    }
}

/// Stores `volume` as the level to restore on the next connection.
///
/// Failures are logged rather than propagated: losing the remembered volume should never take
/// down a playback command.
pub fn save(volume: u8) {
    let path = state_file_path();
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!("Failed to create cache folder for volume state: {err:#}");
            return;
        }
    }
    if let Err(err) = std::fs::write(&path, volume.to_string()) {
        tracing::warn!("Failed to persist volume to {}: {err:#}", path.display());
    }
}

/// Applies a percentage `offset` to `current`, keeping the result a valid volume.
///
/// Both ends must be clamped. The original implementation capped only the maximum, so decreasing
/// below zero produced a negative number that wrapped into a very large volume when cast to `u8`.
pub fn offset_volume(current: u8, offset: i32) -> u8 {
    let volume = (i32::from(current) + offset).clamp(0, 100);
    u8::try_from(volume).unwrap_or(0)
}

/// The volume a newly created device should start at: the last persisted level, falling back to
/// the configured `device.volume` only when nothing has been stored yet.
// Only called when setting up the integrated player, which is gated behind `streaming`.
#[cfg_attr(not(feature = "streaming"), allow(dead_code))]
pub fn initial() -> u8 {
    let configured = std::cmp::min(config::get_config().app_config.device.volume, 100);
    if let Some(volume) = load() {
        tracing::info!("Restoring persisted volume {volume}%");
        return volume;
    }
    tracing::info!("No persisted volume found; starting at configured {configured}%");
    configured
}

#[cfg(test)]
mod tests {
    use super::offset_volume;

    #[test]
    fn increases_and_decreases_by_the_offset() {
        assert_eq!(offset_volume(70, 5), 75);
        assert_eq!(offset_volume(70, -5), 65);
    }

    #[test]
    fn saturates_at_full_volume() {
        assert_eq!(offset_volume(98, 5), 100);
        assert_eq!(offset_volume(100, 5), 100);
    }

    /// Regression: decreasing past zero used to underflow into a near-max volume.
    #[test]
    fn saturates_at_silence_instead_of_wrapping() {
        assert_eq!(offset_volume(3, -5), 0);
        assert_eq!(offset_volume(0, -5), 0);
        assert_eq!(offset_volume(0, -100), 0);
    }

    #[test]
    fn repeated_steps_walk_all_the_way_down() {
        // Each press applies to the level the previous one produced, which is what the optimistic
        // local update in `PlayerState::apply_volume` guarantees.
        let mut volume = 100;
        for _ in 0..20 {
            volume = offset_volume(volume, -5);
        }
        assert_eq!(volume, 0);
    }
}
