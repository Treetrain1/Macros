use super::persistence::read_macro_by_id;
use super::settings::{load_settings, update_settings};
use crate::macros::Macro;

pub fn set_selected_macro_id(macro_id: Option<&str>) {
    let id = macro_id.map(|s| s.to_string());
    update_settings(|s| s.selected_macro_id = id);
}

pub fn get_selected_macro_id() -> Option<String> {
    load_settings().selected_macro_id
}

pub fn get_macro_by_id(macro_id: &str) -> Option<Macro> {
    read_macro_by_id(macro_id)
}
