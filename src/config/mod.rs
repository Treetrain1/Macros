pub(crate) mod persistence;
pub(crate) mod selection;
pub(crate) mod settings;

pub(crate) use persistence::{get_macros_from_config, migrate_legacy_macros_to_files};
pub(crate) use selection::{get_macro_by_id, get_selected_macro_id, set_selected_macro_id};
pub(crate) use settings::save_config_value;

pub(crate) const APP_ID: &str = "com.treetrain1.Macros";

use crate::macros::Macro;

impl Macro {
    pub(crate) fn add(mut self) -> Result<(), String> {
        self.ensure_id();
        self.save()
    }
}
