use super::instruction_widget::{add_instruction_at, instruction_row};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::macros::Macro;
use cosmic::iced::widget::button;
use cosmic::iced::{Alignment, Length};
use cosmic::Apply;
use cosmic::{widget, Element};

pub(crate) fn macro_editor<'a>(
    mac: &'a Macro,
    confirm_clear_instructions: bool,
    key_capture_index: Option<usize>,
    can_undo: bool,
    can_redo: bool,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let pill_button = |label: &'static str| {
        button(cosmic::widget::text(label)).padding([10, 18])
    };

    let clear_instructions_label = if confirm_clear_instructions {
        "⚠ Confirm clear (5s)"
    } else {
        "⚠ Clear instructions"
    };

    let undo_button = if can_undo {
        pill_button("↩ Undo").on_press(Undo)
    } else {
        pill_button("↩ Undo")
    };
    let redo_button = if can_redo {
        pill_button("↪ Redo").on_press(Redo)
    } else {
        pill_button("↪ Redo")
    };

    let mut instructions: Vec<Element<Message>> = vec![];

    for (index, ins) in mac.code.iter().cloned().enumerate() {
        instructions.push(instruction_row(index, ins, key_capture_index, spacing));
    }

    let len = mac.code.len();
    instructions.push(
        cosmic::widget::dropdown(
            &["Wait", "Text", "Key", "Mouse Button", "Move Mouse", "Scroll", "Run Script", "Comment"],
            None,
            move |selected| add_instruction_at(len, selected),
        ).into()
    );

    widget::settings::view_column(
        vec![
            widget::settings::section()
                .add(
                    widget::column::with_children(vec![
                        widget::text::body("Title").into(),
                        widget::text_input("Macro", &mac.name)
                            .on_input(SetTitle)
                            .into()
                    ])
                    .spacing(spacing.space_xxs)
                    .padding([0, 15, 0, 15]),
                )
                .add(
                    widget::column::with_children(vec![
                        widget::text::body("Instructions").into(),
                        widget::column::with_children(instructions).spacing(spacing.space_xs).into(),
                        cosmic::widget::container(
                            cosmic::widget::row![
                                cosmic::widget::tooltip(
                                    undo_button,
                                    cosmic::widget::container("Undo last instruction change"),
                                    cosmic::widget::tooltip::Position::Top
                                ),
                                cosmic::widget::tooltip(
                                    redo_button,
                                    cosmic::widget::container("Redo last undone change"),
                                    cosmic::widget::tooltip::Position::Top
                                ),
                                cosmic::widget::tooltip(
                                    pill_button(clear_instructions_label).on_press(ClearInstructions),
                                    cosmic::widget::container(if confirm_clear_instructions {
                                        "Click again within 5 seconds to remove every instruction in this macro"
                                    } else {
                                        "Arms removal for every instruction in this macro"
                                    }),
                                    cosmic::widget::tooltip::Position::Top
                                ),
                                cosmic::widget::tooltip(
                                    pill_button("💾 Save macro").on_press(SaveMacro),
                                    cosmic::widget::container("Persist the current macro to your config"),
                                    cosmic::widget::tooltip::Position::Top
                                ),
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center)
                        )
                        .width(Length::Fill)
                        .align_x(Alignment::Center)
                        .into()
                    ])
                    .spacing(spacing.space_xxs)
                    .padding([0, 15, 0, 15])
                    .apply(cosmic::widget::scrollable),
                )
                .into(),
        ]
    )
    .padding(10)
    .width(Length::Fill)
    .height(Length::Shrink)
    .align_x(Alignment::Center)
    .into()
}
