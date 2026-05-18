pub(crate) mod controls;
pub(crate) mod editor;
pub(crate) mod instruction_widget;
pub(crate) mod settings;

use crate::app::message::Message;
use crate::app::state::Page;
use crate::app::App;
use controls::{macro_selector_row, run_controls_row};
use cosmic::iced::{Alignment, Length};
use cosmic::Element;
use editor::macro_editor;

pub(crate) const DEFAULT_WAIT_TIME: u64 = 1000;
pub(crate) const DEFAULT_SCROLL_AMOUNT: i32 = 4;
pub(crate) const CLEAR_CONFIRM_TIMEOUT_SECS: u64 = 5;

pub(crate) fn icon_path(name: &str) -> std::path::PathBuf {
    let installed = std::path::PathBuf::from("/usr/share/macros/icons").join(name);
    if installed.exists() {
        return installed;
    }
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/res/icons/")).join(name)
}

pub(crate) fn build_view(app: &App) -> Element<'_, Message> {
    match app.editor_ui.page {
        Page::Settings => settings::settings_view(app),
        Page::Main => build_main_view(app),
    }
}

fn build_main_view(app: &App) -> Element<'_, Message> {
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
        &app.editor_ui.recording_phase,
        app.editor_ui.record_mouse_relative,
        &spacing,
    ));

    if let Some(mac) = &app.macro_lib.current_macro {
        content = content.push(macro_editor(
            mac,
            app.editor_ui.confirm_clear_instructions,
            app.editor_ui.key_capture_index,
            !app.editor_ui.undo_stack.is_empty(),
            !app.editor_ui.redo_stack.is_empty(),
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
