pub(crate) mod controls;
pub(crate) mod editor;
pub(crate) mod instruction_widget;

use crate::app::message::Message;
use crate::app::App;
use controls::{macro_selector_row, run_controls_row};
use cosmic::iced::{Alignment, Length};
use cosmic::Element;
use editor::macro_editor;

pub(crate) const DEFAULT_WAIT_TIME: u64 = 1000;
pub(crate) const DEFAULT_SCROLL_AMOUNT: i32 = 4;
pub(crate) const CLEAR_CONFIRM_TIMEOUT_SECS: u64 = 5;

pub(crate) const ICON_REMOVE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/icons/remove.svg");
pub(crate) const ICON_UP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/icons/up.svg");
pub(crate) const ICON_DOWN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/icons/down.svg");

pub(crate) fn build_view(app: &App) -> Element<'_, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let has_selected_macro = app.macro_lib.current_macro.is_some();

    let mut content = cosmic::widget::column![];

    content = content.push(macro_selector_row(
        &app.macro_lib.macro_strs,
        app.macro_lib.macro_selected,
        app.editor_ui.confirm_remove_macro,
        &spacing,
    ));

    content = content.push(run_controls_row(
        app.execution.loop_mode_enabled,
        has_selected_macro,
        &spacing,
    ));

    if let Some(mac) = &app.macro_lib.current_macro {
        content = content.push(macro_editor(
            mac,
            app.editor_ui.confirm_clear_instructions,
            app.editor_ui.key_capture_index,
            &spacing,
        ));
    }

    cosmic::widget::container(content)
        .width(Length::Fill)
        .height(Length::Shrink)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}
