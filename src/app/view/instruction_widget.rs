use super::{custom_icon, DEFAULT_SCROLL_AMOUNT, DEFAULT_WAIT_TIME, ICON_RED};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::app::state::FieldId;
use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::ui_utils::{axis_to_index, coordinate_to_index, direction_to_index, index_to_axis, index_to_coordinate, index_to_direction};
use crate::input::{get_mouse_button_names, index_to_mouse_button, key_to_string, mouse_button_to_index};
use crate::macros::Instruction;
use cosmic::iced::widget::button;
use cosmic::iced::Alignment;
use cosmic::{widget, Element};
use std::collections::HashMap;

/// Returns (display_text, is_invalid) for a numeric field. While the field has
/// been typed into, the literal typed text is shown (even once it parses
/// successfully) so reformatting the committed value can't make characters
/// like a trailing "." appear to vanish mid-edit; `T` is used only to check
/// whether that text currently parses, for the error indicator.
fn field_display<T: std::str::FromStr + std::fmt::Display>(
    buffers: &HashMap<(usize, FieldId), String>,
    index: usize,
    field: FieldId,
    committed: T,
) -> (String, bool) {
    match buffers.get(&(index, field)) {
        Some(text) => {
            let invalid = text.parse::<T>().is_err();
            (text.clone(), invalid)
        }
        None => (committed.to_string(), false),
    }
}

fn invalid_field_warning<'a>() -> Element<'a, Message> {
    widget::text("Invalid number")
        .class(cosmic::theme::Text::Color(ICON_RED))
        .size(12)
        .into()
}

/// Zero-size filler used in place of a warning when a field is valid, so the
/// row/column shape around a numeric field never changes between renders.
/// Toggling between two different widget tree shapes (e.g. returning a bare
/// row vs. wrapping it in a column) breaks iced's diffing and silently drops
/// keyboard focus from whatever text input the user was just typing into.
fn empty_field_warning<'a>() -> Element<'a, Message> {
    widget::Space::new()
        .width(cosmic::iced::Length::Fixed(0.0))
        .height(cosmic::iced::Length::Fixed(0.0))
        .into()
}

fn field_warning<'a>(invalid: bool) -> Element<'a, Message> {
    if invalid { invalid_field_warning() } else { empty_field_warning() }
}

pub(crate) fn instruction_row<'a>(
    index: usize,
    ins: Instruction,
    key_capture_index: Option<usize>,
    spacing: &cosmic::cosmic_theme::Spacing,
    invalid_field_buffers: &HashMap<(usize, FieldId), String>,
) -> Element<'a, Message> {
    let compact_icon_button = |name: &str| {
        widget::button::icon(custom_icon(name)).padding(8)
    };

    let instruction: Element<Message> = match ins {
        Instruction::Token(token) => {
            match token {
                InputToken::Text(text) => {
                    cosmic::widget::row![
                        widget::text::body("Text:".to_string()).align_y(Alignment::Center),
                        widget::text_input("", text)
                            .on_input(move |x| EditInstruction(index, Instruction::Token(InputToken::Text(x)))),
                    ].spacing(10).into()
                }
                InputToken::Key(key, direction) => {
                    let key_label = if key_capture_index == Some(index) {
                        "Press any key...".to_string()
                    } else {
                        format!("{}", key_to_string(&key).unwrap_or("Unknown"))
                    };
                    let dir_idx = direction_to_index(&direction);

                    cosmic::widget::row![
                        widget::text::body("Key:".to_string()).align_y(Alignment::Center),
                        button(cosmic::widget::text(key_label))
                            .on_press(StartKeyCapture(index))
                            .width(cosmic::iced::Length::Fill),
                        widget::dropdown(
                            &["Click", "Press", "Release"],
                            Some(dir_idx),
                            move |x: usize| EditInstruction(index, Instruction::Token(InputToken::Key(key.clone(), index_to_direction(x))))
                        ),
                    ].spacing(10).width(cosmic::iced::Length::Fill).into()
                }
                InputToken::Raw(keycode, _) => {
                    widget::text::body(format!("Raw: {:?}", keycode)).into()
                }
                InputToken::Button(btn, direction) => {
                    let btn_idx = mouse_button_to_index(&btn);
                    let dir_idx = direction_to_index(&direction);

                    cosmic::widget::row![
                        widget::text::body("Mouse:".to_string()).align_y(Alignment::Center),
                        widget::dropdown(
                            get_mouse_button_names(),
                            Some(btn_idx),
                            move |x: usize| EditInstruction(index, Instruction::Token(InputToken::Button(index_to_mouse_button(x), direction.clone())))
                        ),
                        widget::dropdown(
                            &["Click", "Press", "Release"],
                            Some(dir_idx),
                            move |x: usize| EditInstruction(index, Instruction::Token(InputToken::Button(btn.clone(), index_to_direction(x))))
                        ),
                    ].spacing(10).width(cosmic::iced::Length::Fill).into()
                }
                InputToken::MoveMouse(x, y, coordinate) => {
                    let coord_idx = coordinate_to_index(&coordinate);
                    let (x_text, x_invalid) = field_display(invalid_field_buffers, index, FieldId::MoveMouseX, x);
                    let (y_text, y_invalid) = field_display(invalid_field_buffers, index, FieldId::MoveMouseY, y);

                    let mut x_input = widget::text_input("X", x_text)
                        .on_input(move |new_x| EditInstructionField(index, FieldId::MoveMouseX, new_x));
                    if x_invalid {
                        x_input = x_input.error("Invalid number");
                    }
                    let mut y_input = widget::text_input("Y", y_text)
                        .on_input(move |new_y| EditInstructionField(index, FieldId::MoveMouseY, new_y));
                    if y_invalid {
                        y_input = y_input.error("Invalid number");
                    }

                    let main_row = cosmic::widget::row![
                        widget::text::body("Move mouse:".to_string()).align_y(Alignment::Center),
                        x_input,
                        y_input,
                        widget::dropdown(
                            &["Absolute", "Relative"],
                            Some(coord_idx),
                            move |coord: usize| EditInstruction(index, Instruction::Token(InputToken::MoveMouse(x, y, index_to_coordinate(coord))))
                        ),
                    ].spacing(10);

                    cosmic::widget::column![
                        main_row,
                        cosmic::widget::row![field_warning(x_invalid), field_warning(y_invalid)].spacing(10),
                    ].spacing(2).into()
                }
                InputToken::Scroll(amount, axis) => {
                    let axis_idx = axis_to_index(&axis);
                    let (amount_text, amount_invalid) = field_display(invalid_field_buffers, index, FieldId::ScrollAmount, amount);

                    let mut amount_input = widget::text_input("Amount", amount_text)
                        .on_input(move |new_amount| EditInstructionField(index, FieldId::ScrollAmount, new_amount));
                    if amount_invalid {
                        amount_input = amount_input.error("Invalid number");
                    }

                    let main_row = cosmic::widget::row![
                        widget::text::body("Scroll:".to_string()).align_y(Alignment::Center),
                        amount_input,
                        widget::dropdown(
                            &["Vertical", "Horizontal"],
                            Some(axis_idx),
                            move |new_axis: usize| EditInstruction(index, Instruction::Token(InputToken::Scroll(amount, index_to_axis(new_axis))))
                        ),
                    ].spacing(10);

                    cosmic::widget::column![
                        main_row,
                        cosmic::widget::row![field_warning(amount_invalid)].spacing(10),
                    ].spacing(2).into()
                }
                _ => {
                    widget::text::body("Token not implemented").into()
                }
            }
        }
        Instruction::Wait(duration, randomness) => {
            let (duration_text, duration_invalid) = field_display(invalid_field_buffers, index, FieldId::WaitDuration, duration);
            let (randomness_text, randomness_invalid) = field_display(invalid_field_buffers, index, FieldId::WaitRandomness, randomness);

            let mut duration_input = widget::text_input("", duration_text)
                .on_input(move |x| EditInstructionField(index, FieldId::WaitDuration, x));
            if duration_invalid {
                duration_input = duration_input.error("Invalid number");
            }
            let mut randomness_input = widget::text_input("0", randomness_text)
                .on_input(move |x| EditInstructionField(index, FieldId::WaitRandomness, x));
            if randomness_invalid {
                randomness_input = randomness_input.error("Invalid number");
            }

            let main_row = cosmic::widget::row![
                widget::text::body("Wait (ms):".to_string()).align_y(Alignment::Center),
                duration_input,
                widget::text::body("Random ±".to_string()).align_y(Alignment::Center),
                randomness_input,
            ].spacing(10);

            cosmic::widget::column![
                main_row,
                cosmic::widget::row![field_warning(duration_invalid), field_warning(randomness_invalid)].spacing(10),
            ].spacing(2).into()
        }
        Instruction::Command(command) => {
            cosmic::widget::row![
                widget::text::body("Command:".to_string()).align_y(Alignment::Center),
                widget::text_input("bash -c …", command)
                    .on_input(move |x| EditInstruction(index, Instruction::Command(x))),
            ].spacing(10).into()
        }
        Instruction::Comment(text) => {
            cosmic::widget::row![
                widget::text::body("//".to_string()).align_y(Alignment::Center),
                widget::text_input("Comment", text)
                    .on_input(move |x| EditInstruction(index, Instruction::Comment(x))),
            ].spacing(10).into()
        }
    };

    cosmic::widget::row![
        instruction,
        cosmic::widget::container(
            cosmic::widget::row![
                cosmic::widget::tooltip(
                    compact_icon_button("up.svg").on_press(ReorderInstruction(index, -1)),
                    cosmic::widget::container("Move up"),
                    cosmic::widget::tooltip::Position::Top
                ),
                cosmic::widget::tooltip(
                    compact_icon_button("down.svg").on_press(ReorderInstruction(index, 1)),
                    cosmic::widget::container("Move down"),
                    cosmic::widget::tooltip::Position::Bottom
                ),
                cosmic::widget::tooltip(
                    compact_icon_button("remove.svg").on_press(RemoveInstruction(index as isize)),
                    cosmic::widget::container("Remove instruction"),
                    cosmic::widget::tooltip::Position::Left
                ),
                cosmic::widget::dropdown(
                    &["Wait", "Text", "Key", "Mouse Button", "Move Mouse", "Scroll", "Command", "Comment"],
                    None,
                    move |selected| add_instruction_at(index, selected),
                )
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center)
        )
        .width(cosmic::iced::Length::Fill)
        .align_x(Alignment::Center)
    ]
    .spacing(spacing.space_xs)
    .width(cosmic::iced::Length::Fill)
    .into()
}

pub(crate) fn add_instruction_at(index: usize, selected: usize) -> Message {
    match selected {
        0 => AddInstruction(index, Instruction::Wait(DEFAULT_WAIT_TIME, 0.0)),
        1 => AddInstruction(index, Instruction::Token(InputToken::Text("text".into()))),
        2 => AddInstruction(index, Instruction::Token(InputToken::Key(MacroKey::Unicode('a'), Direction::Click))),
        3 => AddInstruction(index, Instruction::Token(InputToken::Button(MacroButton::Left, Direction::Click))),
        4 => AddInstruction(index, Instruction::Token(InputToken::MoveMouse(0, 0, Coordinate::Rel))),
        5 => AddInstruction(index, Instruction::Token(InputToken::Scroll(DEFAULT_SCROLL_AMOUNT, Axis::Vertical))),
        6 => AddInstruction(index, Instruction::Command("".into())),
        7 => AddInstruction(index, Instruction::Comment(String::new())),
        _ => unreachable!(),
    }
}
