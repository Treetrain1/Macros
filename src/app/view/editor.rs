use super::instruction_widget::{add_instruction_at, instruction_row};
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::macros::Macro;
use cosmic::iced::widget::button;
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};

/// Estimated pixel height per instruction row including column spacing.
const ESTIMATED_ROW_HEIGHT: f32 = 60.0;
/// Extra rows rendered above and below the visible area to prevent flicker.
const BUFFER: usize = 5;

pub(crate) fn macro_editor<'a>(
    mac: &'a Macro,
    confirm_clear_instructions: bool,
    key_capture_index: Option<usize>,
    can_undo: bool,
    can_redo: bool,
    spacing: &cosmic::cosmic_theme::Spacing,
    scroll_offset_y: f32,
    scroll_viewport_height: f32,
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

    let len = mac.code.len();

    // Determine the visible slice of instructions to render.
    let viewport_h = if scroll_viewport_height > 0.0 {
        scroll_viewport_height
    } else {
        600.0
    };
    let raw_start = (scroll_offset_y / ESTIMATED_ROW_HEIGHT).floor() as usize;
    let start_idx = raw_start.saturating_sub(BUFFER);
    let visible_count = (viewport_h / ESTIMATED_ROW_HEIGHT).ceil() as usize + 1;
    let end_idx = (raw_start + visible_count + BUFFER).min(len);
    // Captured by the on_scroll closure to suppress messages when the rendered
    // range hasn't changed. Without this, every scroll pixel fires a message.
    let captured_raw_start = raw_start;

    let top_height = start_idx as f32 * ESTIMATED_ROW_HEIGHT;
    let bottom_height = len.saturating_sub(end_idx) as f32 * ESTIMATED_ROW_HEIGHT;

    let mut instructions: Vec<Element<Message>> = vec![];

    if top_height > 0.0 {
        instructions.push(
            widget::Space::new().width(Length::Fill).height(Length::Fixed(top_height)).into(),
        );
    }

    for (index, ins) in mac.code.iter().cloned().enumerate().skip(start_idx).take(end_idx - start_idx) {
        instructions.push(instruction_row(index, ins, key_capture_index, spacing));
    }

    if bottom_height > 0.0 {
        instructions.push(
            widget::Space::new().width(Length::Fill).height(Length::Fixed(bottom_height)).into(),
        );
    }

    instructions.push(
        cosmic::widget::dropdown(
            &["Wait", "Text", "Key", "Mouse Button", "Move Mouse", "Scroll", "Run Script", "Comment"],
            None,
            move |selected| add_instruction_at(len, selected),
        ).into()
    );

    let instructions_col = widget::column::with_children(vec![
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
    .padding([0, 15, 0, 15]);

    let scrollable = cosmic::widget::scrollable(instructions_col)
        .id(cosmic::widget::Id::new("macro-editor-scroll"))
        .on_scroll(move |vp| {
            let new_y = vp.absolute_offset().y;
            let new_raw = (new_y / ESTIMATED_ROW_HEIGHT).floor() as usize;
            if new_raw == captured_raw_start {
                NoOp
            } else {
                EditorScrolled(new_y, vp.bounds().height)
            }
        });

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
                .add(scrollable)
                .into(),
        ]
    )
    .padding(10)
    .width(Length::Fill)
    .height(Length::Shrink)
    .align_x(Alignment::Center)
    .into()
}
