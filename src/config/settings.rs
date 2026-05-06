use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use tracing::warn;

pub(crate) fn save_config_value<T: serde::Serialize>(config: &Config, key: &str, value: T) -> Result<(), String> {
    config.set(key, value).map_err(|err| {
        let error_msg = format!("Failed to save {} to config: {}", key, err);
        warn!("{}", error_msg);
        error_msg
    })
}

pub(crate) fn get_config_value<T: serde::de::DeserializeOwned>(config: &Config, key: &str) -> Option<T> {
    config.get::<T>(key).ok()
}
