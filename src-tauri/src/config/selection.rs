use super::persistence::get_macros_from_config;
use super::settings::{load_settings, update_settings};
use crate::macros::Macro;

pub(crate) fn set_selected_macro_id(macro_id: Option<&str>) {
    let id = macro_id.map(|s| s.to_string());
    update_settings(|s| s.selected_macro_id = id);
}

pub(crate) fn get_selected_macro_id() -> Option<String> {
    load_settings().selected_macro_id
}

pub(crate) fn get_macro_by_id(macro_id: &str) -> Option<Macro> {
    get_macros_from_config()
        .into_iter()
        .find(|mac| mac.id == macro_id)
}
