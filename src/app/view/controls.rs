use super::icon_path;
use crate::app::message::Message;
use crate::app::message::Message::*;
use cosmic::iced::widget::button;
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};
use std::path::PathBuf;

pub(crate) fn macro_selector_row<'a>(
    macro_strs: &'a [String],
    macro_selected: Option<usize>,
    confirm_remove_macro: bool,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let compact_icon_button = |path: PathBuf| {
        widget::button::icon(widget::icon::from_path(path)).padding(8)
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
    let remove_macro_button = if has_selected {
        compact_icon_button(icon_path("remove.svg")).on_press(RemoveMacro)
    } else {
        compact_icon_button(icon_path("remove.svg"))
    };

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
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let pill_button = |label: &'static str| {
        button(cosmic::widget::text(label)).padding([10, 18])
    };

    let run_macro_label = if loop_mode_enabled {
        "⟲ Start loop"
    } else {
        "▶ Run macro"
    };

    let run_macro_button = if has_selected_macro {
        pill_button(run_macro_label).on_press(RunMacro)
    } else {
        pill_button(run_macro_label)
    };

    #[cfg(target_os = "linux")]
    let loop_mode = cosmic::widget::checkbox(loop_mode_enabled)
        .name("Loop mode")
        .on_toggle(ToggleLoopMode);
    #[cfg(not(target_os = "linux"))]
    let loop_mode = cosmic::widget::checkbox(loop_mode_enabled)
        .on_toggle(ToggleLoopMode);

    cosmic::widget::row![
        cosmic::widget::container(
            cosmic::widget::row![
                cosmic::widget::tooltip(
                    run_macro_button,
                    cosmic::widget::container("Runs the current macro once or starts looping if enabled"),
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
            .align_y(Alignment::Center)
        )
        .width(Length::Fill)
        .align_x(Alignment::Center),
        cosmic::widget::container(cosmic::widget::text("")).width(Length::Fill),
        cosmic::widget::container(cosmic::widget::text("")).width(Length::Fill).align_x(Alignment::Center),
    ]
    .spacing(spacing.space_s)
    .width(Length::Fill)
    .into()
}
