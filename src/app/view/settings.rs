use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::config;
use crate::app::state::ComboCapture;
use crate::app::App;
use crate::hotkey_types::{HotkeyAction, HotkeyBinding, KeyCombo};
use cosmic::iced::widget::button;
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};

pub(crate) fn settings_view(app: &App) -> Element<'_, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let bindings = &app.editor_ui.hotkey_bindings;

    let yellow = cosmic::iced::Color { r: 0.95, g: 0.75, b: 0.1, a: 1.0 };
    let grab_warning: Option<cosmic::Element<Message>> = if crate::recording::grab_failed() {
        Some(
            widget::column![
                widget::text("⚠ Global hotkeys unavailable: user not in \"input\" group.")
                    .class(cosmic::theme::Text::Color(yellow)),
                widget::text("Run: sudo usermod -aG input $USER")
                    .class(cosmic::theme::Text::Color(yellow)),
            ]
            .spacing(spacing.space_xxs)
            .into(),
        )
    } else {
        None
    };

    let named_actions: &[(&str, HotkeyAction)] = &[
        ("Run Macro", HotkeyAction::RunMacro),
        ("Stop Loop", HotkeyAction::StopLoop),
        ("Next Macro", HotkeyAction::NextMacro),
        ("Previous Macro", HotkeyAction::PrevMacro),
        ("Toggle Loop", HotkeyAction::ToggleLoop),
    ];

    let mut named_rows: Vec<Element<Message>> = vec![];
    for (label, action) in named_actions {
        let combo = bindings
            .iter()
            .find(|b| &b.action == action)
            .map(|b| b.combo.format());
        let is_capturing = app.editor_ui.combo_capture
            == Some(ComboCapture::Named(action.clone()));
        named_rows.push(named_hotkey_row(label, action.clone(), combo, is_capturing));
    }

    // Per-macro hotkey rows — build with owned data to avoid lifetime issues
    let macros = crate::config::get_macros_from_config(&app.config);
    let per_macro_entries: Vec<(usize, String, String)> = bindings
        .iter()
        .enumerate()
        .filter_map(|(idx, b)| {
            if let HotkeyAction::RunSpecificMacro(ref macro_id) = b.action {
                let macro_name = macros
                    .iter()
                    .find(|m| &m.id == macro_id)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| "(deleted)".to_string());
                Some((idx, macro_name, b.combo.format()))
            } else {
                None
            }
        })
        .collect();

    let mut per_macro_rows: Vec<Element<Message>> = per_macro_entries
        .into_iter()
        .map(|(binding_idx, macro_name, combo_str)| {
            widget::row![
                widget::text(macro_name)
                    .width(Length::Fill)
                    .align_x(Alignment::Start),
                button(widget::text(combo_str))
                    .padding([6, 12])
                    .on_press(StartComboCapture(HotkeyAction::RunSpecificMacro(
                        String::new()
                    ))),
                button(widget::text("✕"))
                    .padding([6, 10])
                    .on_press(RemoveHotkeyBinding(binding_idx)),
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    per_macro_rows.push(build_add_form(app, &spacing));

    let back_btn = button(widget::text("← Back"))
        .padding([10, 18])
        .on_press(CloseSettings);

    let settings_col = widget::settings::view_column(vec![
        widget::settings::section()
            .title("Global Hotkeys")
            .add(
                widget::column::with_children(named_rows)
                    .spacing(spacing.space_xs)
                    .padding([0, 15, 0, 15]),
            )
            .into(),
        widget::settings::section()
            .title("Per-Macro Hotkeys")
            .add(
                widget::column::with_children(per_macro_rows)
                    .spacing(spacing.space_s)
                    .padding([0, 15, 0, 15]),
            )
            .into(),
    ])
    .width(Length::Fill);

    let mut content_children: Vec<Element<Message>> = vec![
        widget::row![back_btn].spacing(spacing.space_s).into(),
    ];
    if let Some(w) = grab_warning {
        content_children.push(w);
    }
    content_children.push(settings_col.into());

    let content = widget::column::with_children(content_children)
        .spacing(spacing.space_s)
        .padding(spacing.space_s);

    widget::container(content)
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}

fn named_hotkey_row<'a>(
    label: &'a str,
    action: HotkeyAction,
    combo_str: Option<String>,
    is_capturing: bool,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let combo_btn: Element<Message> = if is_capturing {
        button(widget::text("Press combo..."))
            .padding([6, 12])
            .into()
    } else {
        let btn_label = combo_str
            .clone()
            .unwrap_or_else(|| "Not set".to_string());
        button(widget::text(btn_label))
            .padding([6, 12])
            .on_press(StartComboCapture(action.clone()))
            .into()
    };

    let default_combo = config::default_combo_for_action(&action);
    let already_default = combo_str.as_deref()
        == default_combo.as_ref().map(|c| c.format()).as_deref();

    let default_btn: Element<Message> = if !is_capturing && !already_default {
        button(widget::text("Default"))
            .padding([6, 10])
            .on_press(ResetHotkeyToDefault(action.clone()))
            .into()
    } else {
        widget::text("").into()
    };

    let clear_btn: Element<Message> = if combo_str.is_some() && !is_capturing {
        button(widget::text("✕"))
            .padding([6, 10])
            .on_press(ClearNamedHotkey(action))
            .into()
    } else {
        widget::text("").into()
    };

    widget::row![
        widget::text(label)
            .width(Length::Fill)
            .align_x(Alignment::Start),
        combo_btn,
        default_btn,
        clear_btn,
    ]
    .spacing(spacing.space_xs)
    .align_y(Alignment::Center)
    .into()
}

fn build_add_form<'a>(
    app: &'a App,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let pending = &app.editor_ui.pending_macro_hotkey;
    let selected_idx = pending.as_ref().and_then(|(idx, _)| *idx);
    let pending_combo_str = pending
        .as_ref()
        .and_then(|(_, c)| c.as_ref())
        .map(|c| c.format());
    let is_capturing_pending = app.editor_ui.combo_capture == Some(ComboCapture::Pending);

    let macro_dropdown = widget::dropdown(
        &app.macro_lib.macro_strs,
        selected_idx,
        |idx| SetPendingMacroIdx(Some(idx)),
    );

    let combo_btn: Element<Message> = if is_capturing_pending {
        button(widget::text("Press combo..."))
            .padding([6, 12])
            .into()
    } else {
        let label = pending_combo_str.unwrap_or_else(|| "Set combo".to_string());
        button(widget::text(label))
            .padding([6, 12])
            .on_press(StartPendingComboCapture)
            .into()
    };

    let can_add = selected_idx.is_some()
        && pending
            .as_ref()
            .and_then(|(_, c)| c.as_ref())
            .is_some();

    let add_btn = if can_add {
        button(widget::text("＋ Add"))
            .padding([6, 14])
            .on_press(AddMacroHotkey)
    } else {
        button(widget::text("＋ Add")).padding([6, 14])
    };

    widget::row![
        widget::container(macro_dropdown).width(Length::Fill),
        combo_btn,
        add_btn,
    ]
    .spacing(spacing.space_xs)
    .align_y(Alignment::Center)
    .into()
}
