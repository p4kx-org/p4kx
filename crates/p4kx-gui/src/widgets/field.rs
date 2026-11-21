// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::epaint::RectShape;
use egui::layers::ShapeIdx;
use egui::{Color32, Label, Rect, Sense, Shape, Stroke, StrokeKind, Ui, vec2};

use super::indicator::warning_icon;
use super::text::plain;
use crate::theme::palette;
use crate::theme::tokens::{CORNER_CONTROL, FS_XS, GLOW_PRIMARY, REM, Weight};

/// The `margin-top` of `.field-error`.
const ERROR_TOP: f32 = 0.4 * REM;
/// The `gap` of `.field-error` between the warning glyph and the text.
const ERROR_GAP: f32 = 0.35 * REM;

/// Reserves the place behind a text field where its fill and border are painted once the field
/// has its rectangle.
pub(crate) fn frame_slot(ui: &Ui) -> ShapeIdx {
    ui.painter().add(Shape::Noop)
}

/// Reserves the place behind a text field where its bloom is painted while it has focus.
pub(crate) fn bloom_slot(ui: &Ui) -> ShapeIdx {
    ui.painter().add(Shape::Noop)
}

/// Fills `slot` with the `input[type="text"]` frame at `rect`: the control fill and a
/// `control-border` border that turns `color-primary` when `highlighted`.
pub(crate) fn paint_frame(ui: &Ui, slot: ShapeIdx, rect: Rect, highlighted: bool) {
    let border = if highlighted {
        palette::PRIMARY
    } else {
        palette::CONTROL_BORDER
    };
    paint(ui, slot, rect, border);
}

/// Fills `slot` with the frame of a field that holds an error: the same fill with a
/// `color-danger` border, as `border-color: var(--color-danger)` on the input.
pub(crate) fn paint_error_frame(ui: &Ui, slot: ShapeIdx, rect: Rect) {
    paint(ui, slot, rect, palette::DANGER);
}

fn paint(ui: &Ui, slot: ShapeIdx, rect: Rect, border: Color32) {
    ui.painter().set(
        slot,
        RectShape::new(
            rect,
            CORNER_CONTROL,
            palette::CONTROL_BG,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        ),
    );
}

/// Fills `slot` with the `glow-primary` bloom around `rect`.
pub(crate) fn paint_bloom(ui: &Ui, slot: ShapeIdx, rect: Rect) {
    ui.painter()
        .set(slot, GLOW_PRIMARY.as_shape(rect, CORNER_CONTROL));
}

/// Shows `text` under a field as `.field-error`: `fs-xs` in `color-danger-text` with a warning
/// glyph in front, a little below the field.
pub(crate) fn error_note(ui: &mut Ui, text: &str) {
    ui.add_space(ERROR_TOP);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = ERROR_GAP;
        let (icon, _) = ui.allocate_exact_size(vec2(FS_XS, FS_XS), Sense::hover());
        warning_icon(ui, icon, palette::DANGER_TEXT);
        ui.add(Label::new(plain(text, FS_XS, Weight::Regular).color(palette::DANGER_TEXT)).wrap());
    });
}
