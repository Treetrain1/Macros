use super::instruction_widget::{add_instruction_at, instruction_row};
use super::icon_label_button;
use crate::app::message::Message;
use crate::app::message::Message::*;
use crate::app::state::FieldId;
use crate::macros::Macro;
use cosmic::iced::widget::button;
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};
use std::collections::HashMap;

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
    invalid_field_buffers: &'a HashMap<(usize, FieldId), String>,
) -> Element<'a, Message> {
    let clear_instructions_label = if confirm_clear_instructions {
        "Confirm clear (5s)"
    } else {
        "Clear instructions"
    };

    let undo_button = {
        let b = widget::button::icon(widget::icon::from_name("edit-undo-symbolic"))
            .padding(8);
        if can_undo { b.on_press(Undo) } else { b }
    };
    let redo_button = {
        let b = widget::button::icon(widget::icon::from_name("edit-redo-symbolic"))
            .padding(8);
        if can_redo { b.on_press(Redo) } else { b }
    };

    let len = mac.code.len();

    // Determine the visible slice of instructions to render.
    let viewport_h = if scroll_viewport_height > 0.0 {
        scroll_viewport_height
    } else {
        600.0
    };
    let raw_start = (scroll_offset_y / ESTIMATED_ROW_HEIGHT).floor() as usize;
    let visible_count = (viewport_h / ESTIMATED_ROW_HEIGHT).ceil() as usize + 1;
    let end_idx = (raw_start + visible_count + BUFFER).min(len);
    // Clamp start to end so that stale scroll positions (e.g. after undo or
    // stop-recording shrinks the list) never produce a usize underflow.
    let start_idx = raw_start.saturating_sub(BUFFER).min(end_idx);
    // Captured by the on_scroll closure to suppress messages when neither the
    // rendered range nor the viewport height has changed.
    let captured_raw_start = raw_start;
    let captured_viewport_h = scroll_viewport_height;

    let top_height = start_idx as f32 * ESTIMATED_ROW_HEIGHT;
    let bottom_height = len.saturating_sub(end_idx) as f32 * ESTIMATED_ROW_HEIGHT;

    let mut instructions: Vec<Element<Message>> = vec![];

    if top_height > 0.0 {
        instructions.push(
            widget::Space::new().width(Length::Fill).height(Length::Fixed(top_height)).into(),
        );
    }

    for (index, ins) in mac.code.iter().cloned().enumerate().skip(start_idx).take(end_idx - start_idx) {
        instructions.push(instruction_row(index, ins, key_capture_index, spacing, invalid_field_buffers));
    }

    if bottom_height > 0.0 {
        instructions.push(
            widget::Space::new().width(Length::Fill).height(Length::Fixed(bottom_height)).into(),
        );
    }

    instructions.push(
        cosmic::widget::dropdown(
            &["Wait", "Text", "Key", "Mouse Button", "Move Mouse", "Scroll", "Command", "Comment"],
            None,
            move |selected| add_instruction_at(len, selected),
        ).into()
    );

    let instructions_col = widget::column::with_children(vec![
        widget::text::body("Instructions").into(),
        widget::column::with_children(instructions).spacing(spacing.space_xs).into(),
        cosmic::widget::container(
            cosmic::widget::row![
                undo_button,
                redo_button,
                icon_label_button("dialog-warning-symbolic", clear_instructions_label, 6, Some(ClearInstructions)),
                icon_label_button("document-save-symbolic", "Save macro", 6, Some(SaveMacro)),
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
            let new_h = vp.bounds().height;
            // Only fire when the rendered row range would actually change: either
            // the scroll position moved to a new row, or the viewport grew/shrank
            // enough that visible_count changes. Comparing row counts (not raw
            // pixel heights) avoids sending a message on every resize pixel.
            let new_visible = (new_h / ESTIMATED_ROW_HEIGHT).ceil() as usize;
            let old_visible = (captured_viewport_h / ESTIMATED_ROW_HEIGHT).ceil() as usize;
            if new_raw == captured_raw_start && new_visible == old_visible {
                NoOp
            } else {
                EditorScrolled(new_y, new_h)
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
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}
