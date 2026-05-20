use super::{icon_path, DEFAULT_SCROLL_AMOUNT, DEFAULT_WAIT_TIME};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::input::ui_utils::{axis_to_index, coordinate_to_index, direction_to_index, index_to_axis, index_to_coordinate, index_to_direction};
use crate::input::{get_mouse_button_names, index_to_mouse_button, key_to_string, mouse_button_to_index};
use crate::macros::Instruction;
use cosmic::iced::widget::button;
use cosmic::iced::Alignment;
use cosmic::{widget, Element};
use enigo::agent::Token;
use enigo::{Axis, Coordinate, Direction};
use std::path::PathBuf;


pub(crate) fn instruction_row<'a>(
    index: usize,
    ins: Instruction,
    key_capture_index: Option<usize>,
    spacing: &cosmic::cosmic_theme::Spacing,
) -> Element<'a, Message> {
    let compact_icon_button = |path: PathBuf| {
        widget::button::icon(widget::icon::from_path(path)).padding(8)
    };

    let instruction: Element<Message> = match ins {
        Instruction::Token(token) => {
            match token {
                Token::Text(text) => {
                    cosmic::widget::row![
                        widget::text::body("Text:".to_string()).align_y(Alignment::Center),
                        widget::text_input("", text)
                            .on_input(move |x| EditInstruction(index, Instruction::Token(Token::Text(x)))),
                    ].spacing(10).into()
                }
                Token::Key(key, direction) => {
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
                            move |x: usize| EditInstruction(index, Instruction::Token(Token::Key(key.clone(), index_to_direction(x))))
                        ),
                    ].spacing(10).width(cosmic::iced::Length::Fill).into()
                }
                Token::Raw(keycode, _) => {
                    widget::text::body(format!("Raw: {:?}", keycode)).into()
                }
                Token::Button(btn, direction) => {
                    let btn_idx = mouse_button_to_index(&btn);
                    let dir_idx = direction_to_index(&direction);

                    cosmic::widget::row![
                        widget::text::body("Mouse:".to_string()).align_y(Alignment::Center),
                        widget::dropdown(
                            get_mouse_button_names(),
                            Some(btn_idx),
                            move |x: usize| EditInstruction(index, Instruction::Token(Token::Button(index_to_mouse_button(x), direction.clone())))
                        ),
                        widget::dropdown(
                            &["Click", "Press", "Release"],
                            Some(dir_idx),
                            move |x: usize| EditInstruction(index, Instruction::Token(Token::Button(btn, index_to_direction(x))))
                        ),
                    ].spacing(10).width(cosmic::iced::Length::Fill).into()
                }
                Token::MoveMouse(x, y, coordinate) => {
                    let coord_idx = coordinate_to_index(&coordinate);

                    cosmic::widget::row![
                        widget::text::body("Move mouse:".to_string()).align_y(Alignment::Center),
                        widget::text_input("X", format!("{}", x))
                            .on_input(move |new_x| EditInstruction(index, Instruction::Token(Token::MoveMouse(new_x.parse().unwrap_or(x), y, coordinate.clone())))),
                        widget::text_input("Y", format!("{}", y))
                            .on_input(move |new_y| EditInstruction(index, Instruction::Token(Token::MoveMouse(x, new_y.parse().unwrap_or(y), coordinate.clone())))),
                        widget::dropdown(
                            &["Absolute", "Relative"],
                            Some(coord_idx),
                            move |coord: usize| EditInstruction(index, Instruction::Token(Token::MoveMouse(x, y, index_to_coordinate(coord))))
                        ),
                    ].spacing(10).into()
                }
                Token::Scroll(amount, axis) => {
                    let axis_idx = axis_to_index(&axis);

                    cosmic::widget::row![
                        widget::text::body("Scroll:".to_string()).align_y(Alignment::Center),
                        widget::text_input("Amount", format!("{}", amount))
                            .on_input(move |new_amount| EditInstruction(index, Instruction::Token(Token::Scroll(new_amount.parse().unwrap_or(amount), axis.clone())))),
                        widget::dropdown(
                            &["Vertical", "Horizontal"],
                            Some(axis_idx),
                            move |new_axis: usize| EditInstruction(index, Instruction::Token(Token::Scroll(amount, index_to_axis(new_axis))))
                        ),
                    ].spacing(10).into()
                }
                _ => {
                    widget::text::body("Token not implemented").into()
                }
            }
        }
        Instruction::Wait(duration, randomness) => {
            cosmic::widget::row![
                widget::text::body("Wait (ms):".to_string()).align_y(Alignment::Center),
                widget::text_input("", duration.to_string())
                    .on_input(move |x| EditInstruction(index, Instruction::Wait(x.parse().unwrap_or(duration), randomness))),
                widget::text::body("Random ±".to_string()).align_y(Alignment::Center),
                widget::text_input("0", randomness.to_string())
                    .on_input(move |x| EditInstruction(index, Instruction::Wait(duration, x.parse().unwrap_or(randomness)))),
            ].spacing(10).into()
        }
        Instruction::Script(script) => {
            cosmic::widget::row![
                widget::text::body("Script:".to_string()).align_y(Alignment::Center),
                widget::text_input("", script)
                    .on_input(move |x| EditInstruction(index, Instruction::Script(x))),
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
                    compact_icon_button(icon_path("up.svg")).on_press(ReorderInstruction(index, -1)),
                    cosmic::widget::container("Move up"),
                    cosmic::widget::tooltip::Position::Top
                ),
                cosmic::widget::tooltip(
                    compact_icon_button(icon_path("down.svg")).on_press(ReorderInstruction(index, 1)),
                    cosmic::widget::container("Move down"),
                    cosmic::widget::tooltip::Position::Bottom
                ),
                cosmic::widget::tooltip(
                    compact_icon_button(icon_path("remove.svg")).on_press(RemoveInstruction(index as isize)),
                    cosmic::widget::container("Remove instruction"),
                    cosmic::widget::tooltip::Position::Left
                ),
                cosmic::widget::dropdown(
                    &["Wait", "Text", "Key", "Mouse Button", "Move Mouse", "Scroll", "Run Script", "Comment"],
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
    use enigo::{Axis, Button, Coordinate, Direction, Key};
    use enigo::agent::Token;
    match selected {
        0 => AddInstruction(index, Instruction::Wait(DEFAULT_WAIT_TIME, 0)),
        1 => AddInstruction(index, Instruction::Token(Token::Text("text".into()))),
        2 => AddInstruction(index, Instruction::Token(Token::Key(Key::Unicode('a'), Direction::Click))),
        3 => AddInstruction(index, Instruction::Token(Token::Button(Button::Left, Direction::Click))),
        4 => AddInstruction(index, Instruction::Token(Token::MoveMouse(0, 0, Coordinate::Rel))),
        5 => AddInstruction(index, Instruction::Token(Token::Scroll(DEFAULT_SCROLL_AMOUNT, Axis::Vertical))),
        6 => AddInstruction(index, Instruction::Script("script".into())),
        7 => AddInstruction(index, Instruction::Comment(String::new())),
        _ => unreachable!(),
    }
}
