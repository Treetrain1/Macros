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
        #[cfg(target_os = "linux")]
        let col = widget::column![
            widget::text("⚠ Global hotkeys unavailable: user not in \"input\" group.")
                .class(cosmic::theme::Text::Color(yellow)),
            widget::text("Run: sudo usermod -aG input $USER")
                .class(cosmic::theme::Text::Color(yellow)),
        ];
        #[cfg(target_os = "windows")]
        let col = widget::column![
            widget::text("⚠ Global hotkeys unavailable.")
                .class(cosmic::theme::Text::Color(yellow)),
            widget::text("Try running the app as administrator.")
                .class(cosmic::theme::Text::Color(yellow)),
        ];
        Some(col.spacing(spacing.space_xxs).into())
    } else {
        None
    };
    let emulator_warning: Option<cosmic::Element<Message>> = if crate::macros::runner::emulator_failed() {
        #[cfg(target_os = "linux")]
        let col = widget::column![
            widget::text("⚠ Input emulation unavailable: could not open /dev/uinput.")
                .class(cosmic::theme::Text::Color(yellow)),
            widget::text("Run: sudo usermod -aG input $USER  (then log out and back in)")
                .class(cosmic::theme::Text::Color(yellow)),
        ];
        #[cfg(target_os = "windows")]
        let col = widget::column![
            widget::text("⚠ Input emulation unavailable.")
                .class(cosmic::theme::Text::Color(yellow)),
        ];
        Some(col.spacing(spacing.space_xxs).into())
    } else {
        None
    };

    let named_actions: &[(&str, HotkeyAction)] = &[
        ("Run Macro", HotkeyAction::RunMacro),
        ("Stop Loop", HotkeyAction::StopLoop),
        ("Next Macro", HotkeyAction::NextMacro),
        ("Previous Macro", HotkeyAction::PrevMacro),
        ("Toggle Loop", HotkeyAction::ToggleLoop),
        ("Start Recording (No Countdown)", HotkeyAction::StartRecordingImmediate),
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

    let tcp_rows = build_tcp_server_rows(app, &spacing);

    let mut sections: Vec<Element<Message>> = vec![
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
        widget::settings::section()
            .title("TCP Server")
            .add(
                widget::column::with_children(tcp_rows)
                    .spacing(spacing.space_xs)
                    .padding([0, 15, 0, 15]),
            )
            .into(),
    ];

    #[cfg(windows)]
    {
        let update_rows = build_update_rows(app, &spacing);
        sections.push(
            widget::settings::section()
                .title("Updates")
                .add(
                    widget::column::with_children(update_rows)
                        .spacing(spacing.space_xs)
                        .padding([0, 15, 0, 15]),
                )
                .into(),
        );
    }

    let settings_col = widget::settings::view_column(sections).width(Length::Fill);

    let mut scrollable_children: Vec<Element<Message>> = vec![];
    if let Some(w) = grab_warning {
        scrollable_children.push(w);
    }
    if let Some(w) = emulator_warning {
        scrollable_children.push(w);
    }
    scrollable_children.push(settings_col.into());

    let scrollable = widget::scrollable(
        widget::column::with_children(scrollable_children).spacing(spacing.space_s),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    let content = widget::column![
        widget::row![back_btn].spacing(spacing.space_s),
        scrollable,
    ]
    .spacing(spacing.space_s)
    .padding(spacing.space_s);

    widget::container(content)
        .width(Length::Fill)
        .height(Length::Fill)
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

fn build_tcp_server_rows<'a>(
    app: &'a App,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Vec<Element<'a, Message>> {
    let mut port_input = widget::text_input("Port", &app.editor_ui.ipc_port_text)
        .on_input(SetIpcPortText);
    if app.editor_ui.ipc_port_invalid {
        port_input = port_input.error("Invalid port");
    }

    let is_running = app.execution.ipc_server.is_some();
    let toggle_btn = if is_running {
        button(widget::text("Stop Server"))
            .padding([6, 12])
            .on_press(StopIpcServer)
    } else {
        let btn = button(widget::text("Start Server")).padding([6, 12]);
        if app.editor_ui.ipc_port_invalid {
            btn
        } else {
            btn.on_press(StartIpcServer)
        }
    };

    let status_text = match app.execution.ipc_active_port {
        Some(port) => format!("Listening on 127.0.0.1:{port}"),
        None => "Stopped".to_string(),
    };

    vec![
        widget::row![
            widget::text("Port").width(Length::Fill).align_x(Alignment::Start),
            port_input,
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .into(),
        widget::row![
            widget::text(status_text)
                .width(Length::Fill)
                .align_x(Alignment::Start),
            toggle_btn,
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .into(),
        widget::row![
            widget::text("Automatically start server on app launch")
                .width(Length::Fill)
                .align_x(Alignment::Start),
            widget::checkbox(app.execution.ipc_auto_start).on_toggle(SetIpcAutoStart),
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .into(),
    ]
}

#[cfg(windows)]
fn build_update_rows<'a>(
    app: &'a App,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Vec<Element<'a, Message>> {
    use crate::app::state::UpdateCheckState;

    let current_version = env!("CARGO_PKG_VERSION");

    let (status_text, update_available) = match &app.editor_ui.update_check_state {
        UpdateCheckState::Idle => (String::new(), false),
        UpdateCheckState::Checking => ("Checking for updates...".to_string(), false),
        UpdateCheckState::UpToDate => ("Up to date".to_string(), false),
        UpdateCheckState::UpdateAvailable(v) => (format!("Update available: {v}"), true),
        UpdateCheckState::Applying => ("Installing update...".to_string(), false),
        UpdateCheckState::Error(e) => (format!("Update check failed: {e}"), false),
    };

    let checking_or_applying = matches!(
        app.editor_ui.update_check_state,
        UpdateCheckState::Checking | UpdateCheckState::Applying
    );

    let mut check_btn = button(widget::text("Check for Updates")).padding([6, 12]);
    if !checking_or_applying {
        check_btn = check_btn.on_press(CheckForUpdates);
    }

    let mut rows = vec![
        widget::row![
            widget::text(format!("Current version: {current_version}"))
                .width(Length::Fill)
                .align_x(Alignment::Start),
            check_btn,
        ]
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .into(),
    ];

    if !status_text.is_empty() {
        rows.push(
            widget::row![
                widget::text(status_text)
                    .width(Length::Fill)
                    .align_x(Alignment::Start),
            ]
            .spacing(spacing.space_xs)
            .into(),
        );
    }

    if update_available {
        rows.push(
            widget::row![button(widget::text("Update Now"))
                .padding([6, 12])
                .on_press(ApplyUpdate)]
            .spacing(spacing.space_xs)
            .into(),
        );
    }

    rows
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
