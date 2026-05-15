use super::persistence::get_macros_from_config;
use crate::macros::Macro;
use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use tracing::warn;

const SELECTED_MACRO_ID_KEY: &str = "selected_macro_id";

pub(crate) fn set_selected_macro_id(config: &Config, macro_id: Option<&str>) -> Result<(), String> {
    config
        .set(SELECTED_MACRO_ID_KEY, macro_id.map(|id| id.to_string()))
        .map_err(|err| {
            let error_msg = format!("Failed to save selected macro id: {}", err);
            warn!("{}", error_msg);
            error_msg
        })
}

pub(crate) fn get_selected_macro_id(config: &Config) -> Option<String> {
    config
        .get::<Option<String>>(SELECTED_MACRO_ID_KEY)
        .ok()
        .flatten()
}

pub(crate) fn get_macro_by_id(config: &Config, macro_id: &str) -> Option<Macro> {
    get_macros_from_config(config)
        .into_iter()
        .find(|mac| mac.id == macro_id)
}
