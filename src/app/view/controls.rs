use super::{colored_icon, custom_icon, icon_label_button, ICON_DARK, ICON_RED};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::app::state::RecordingPhase;
use cosmic::iced::widget::button;
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};
use std::borrow::Cow;

pub(crate) fn macro_selector_row<'a>(
    macro_strs: &'a [String],
    macro_selected: Option<usize>,
    confirm_remove_macro: bool,
    remove_confirm_remaining_secs: u8,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let compact_icon_button = |name: &str| {
        widget::button::icon(custom_icon(name)).padding(8)
    };

    let symbol_label_button = |symbol: &'static str, label: &'static str| {
        button(
            cosmic::widget::row![
                cosmic::widget::text(symbol),
                cosmic::widget::text(label),
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
    };

    let has_selected = macro_selected.is_some();

    let new_macro_button = symbol_label_button("＋", "New macro").on_press(NewMacro);
    let remove_label: Cow<'static, str> = if confirm_remove_macro {
        Cow::Owned(format!("Delete ({remove_confirm_remaining_secs}s)"))
    } else {
        Cow::Borrowed("Delete")
    };
    let remove_macro_button = if has_selected {
        button(
            cosmic::widget::row![
                widget::icon::icon(custom_icon("remove.svg")).size(16),
                cosmic::widget::text(remove_label.as_ref()),
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .on_press(RemoveMacro)
    } else {
        button(
            cosmic::widget::row![
                widget::icon::icon(custom_icon("remove.svg")).size(16),
                cosmic::widget::text(remove_label.as_ref()),
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
    };

    let settings_button = widget::button::icon(
        widget::icon::from_name("emblem-system-symbolic")
    ).icon_size(24).padding(8).on_press(OpenSettings);

    cosmic::widget::row![
        cosmic::widget::container(
            cosmic::widget::column![
                cosmic::widget::text("Select macro"),
                cosmic::widget::dropdown(macro_strs, macro_selected, |x: usize| SelectMacro(x))
            ]
            .spacing(spacing.space_xxs)
            .align_x(Alignment::Center)
        )
        .width(Length::Fill)
        .align_x(Alignment::Center),
        cosmic::widget::container(cosmic::widget::text("")).width(Length::Fill),
        cosmic::widget::container(
            cosmic::widget::column![
                cosmic::widget::tooltip(
                    new_macro_button,
                    cosmic::widget::container("Add a new macro"),
                    cosmic::widget::tooltip::Position::Left
                ),
                cosmic::widget::tooltip(
                    remove_macro_button,
                    cosmic::widget::container(if confirm_remove_macro {
                        "Click again to permanently delete the selected macro"
                    } else {
                        "Arms deletion for the selected macro"
                    }),
                    cosmic::widget::tooltip::Position::Left
                ),
                cosmic::widget::tooltip(
                    settings_button,
                    cosmic::widget::container("Open Settings"),
                    cosmic::widget::tooltip::Position::Left
                ),
            ]
            .spacing(12)
            .align_x(Alignment::Center)
        )
        .width(Length::Fill)
        .align_x(Alignment::Center),
    ]
    .spacing(spacing.space_s)
    .width(Length::Fill)
    .into()
}

pub(crate) fn run_controls_row<'a>(
    loop_mode_enabled: bool,
    has_selected_macro: bool,
    recording_phase: &RecordingPhase,
    record_mouse_relative: bool,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let run_icon = if loop_mode_enabled { "media-playlist-repeat-symbolic" } else { "media-playback-start-symbolic" };
    let run_label = if loop_mode_enabled { "Start loop" } else { "Run macro" };

    let run_macro_button = {
        let b = button(
            cosmic::widget::row![
                colored_icon(run_icon, 16, ICON_DARK),
                cosmic::widget::text(run_label),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([10, 18]);
        if has_selected_macro { b.on_press(RunMacro) } else { b }
    };

    let loop_mode = cosmic::widget::checkbox(loop_mode_enabled)
        .on_toggle(ToggleLoopMode);

    #[cfg(target_os = "linux")]
    let loop_mode = loop_mode.name("Loop mode");

    let record_element: Element<'_, Message> = {
        let (rec_icon, rec_color, rec_label): (&str, cosmic::iced::Color, String) = match recording_phase {
            RecordingPhase::Countdown(n) => ("media-record-symbolic", ICON_RED, format!("Recording in {}s...", n)),
            RecordingPhase::Active => ("media-playback-stop-symbolic", ICON_DARK, "Recording (Esc to stop)".to_string()),
            RecordingPhase::Idle => ("media-record-symbolic", ICON_RED, "Record".to_string()),
        };
        let btn = button(
            cosmic::widget::row![
                colored_icon(rec_icon, 16, rec_color),
                cosmic::widget::text(rec_label),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([10, 18]);
        let btn = match recording_phase {
            RecordingPhase::Idle if has_selected_macro => btn.on_press(StartRecording),
            _ => btn,
        };
        let relative_toggle = cosmic::widget::tooltip(
            cosmic::widget::container(
                cosmic::widget::row![
                    cosmic::widget::text("Relative mouse"),
                    cosmic::widget::checkbox(record_mouse_relative)
                        .on_toggle(ToggleRecordMouseRelative),
                ]
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center),
            )
            .padding([8, 12]),
            cosmic::widget::container(
                "Record mouse movement as relative offsets instead of absolute coordinates",
            ),
            cosmic::widget::tooltip::Position::Top,
        );
        cosmic::widget::row![btn/*, relative_toggle*/] // TODO
            .spacing(12)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
    };

    let left_column = cosmic::widget::container(
        cosmic::widget::row![
            cosmic::widget::tooltip(
                run_macro_button,
                cosmic::widget::container(
                    "Runs the current macro once or starts looping if enabled"
                ),
                cosmic::widget::tooltip::Position::Top
            ),
            cosmic::widget::tooltip(
                cosmic::widget::container(
                    cosmic::widget::row![
                        cosmic::widget::text("Loop mode"),
                        loop_mode,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                )
                .padding([8, 12]),
                cosmic::widget::container("Enable to loop the macro continuously"),
                cosmic::widget::tooltip::Position::Top
            )
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center);

    cosmic::widget::row![]
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .push(left_column)
        .push(cosmic::widget::container(cosmic::widget::text("")).width(Length::Fill))
        .push(
            cosmic::widget::container(record_element)
                .width(Length::Fill)
                .align_x(Alignment::Center),
        )
        .into()
}
