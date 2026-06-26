pub(crate) mod controls;
pub(crate) mod editor;
pub(crate) mod instruction_widget;
pub(crate) mod settings;

use crate::app::message::Message;
use crate::app::state::Page;
use crate::app::App;
use controls::{macro_selector_row, run_controls_row};
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};
use editor::macro_editor;

pub(crate) const DEFAULT_WAIT_TIME: f64 = 1000.0;
pub(crate) const DEFAULT_SCROLL_AMOUNT: i32 = 4;
pub(crate) const CLEAR_CONFIRM_TIMEOUT_SECS: u64 = 5;

pub(crate) const ICON_DARK: cosmic::iced::Color = cosmic::iced::Color { r: 0.38, g: 0.38, b: 0.38, a: 1.0 };
pub(crate) const ICON_RED: cosmic::iced::Color = cosmic::iced::Color { r: 0.80, g: 0.11, b: 0.16, a: 1.0 };

pub(crate) fn colored_icon<'a>(name: &str, size: u16, color: cosmic::iced::Color) -> Element<'a, Message> {
    widget::icon::icon(widget::icon::from_name(name).into())
        .size(size)
        .class(cosmic::theme::Svg::custom(move |_| {
            cosmic::iced::widget::svg::Style { color: Some(color) }
        }))
        .into()
}

/// Custom (non-theme) icons used by the app, embedded into the binary at
/// compile time so they keep working when the built executable is moved or
/// distributed without its source tree (the previous implementation loaded
/// these from a path under `CARGO_MANIFEST_DIR`, which only existed on the
/// machine that built the binary).
pub(crate) fn custom_icon(name: &str) -> widget::icon::Handle {
    let bytes: &'static [u8] = match name {
        "up.svg" => include_bytes!("../../../res/icons/up.svg"),
        "down.svg" => include_bytes!("../../../res/icons/down.svg"),
        "remove.svg" => include_bytes!("../../../res/icons/remove.svg"),
        _ => panic!("unknown custom icon: {name}"),
    };
    widget::icon::from_svg_bytes(bytes)
}

pub(crate) fn icon_label_button<'a>(
    icon: &str,
    label: &'static str,
    spacing: u16,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    use cosmic::iced::widget::button;
    let icon_elem: Element<'_, Message> = if icon.ends_with(".svg") {
        widget::icon::icon(custom_icon(icon)).size(16).into()
    } else {
        colored_icon(icon, 16, ICON_DARK)
    };
    let b = button(
        cosmic::widget::row![icon_elem, widget::text(label),]
            .spacing(spacing)
            .align_y(cosmic::iced::Alignment::Center),
    )
    .padding([8, 12]);
    match on_press {
        Some(msg) => b.on_press(msg).into(),
        None => b.into(),
    }
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
            app.editor_ui.scroll_offset_y,
            app.editor_ui.scroll_viewport_height,
            &app.editor_ui.invalid_field_buffers,
        ));
    }

    cosmic::widget::container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Start)
        .into()
}
