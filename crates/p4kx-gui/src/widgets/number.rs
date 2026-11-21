// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::RangeInclusive;

use egui::{DragValue, Response, Shape, Stroke, Ui, vec2};

use crate::theme::palette;
use crate::theme::tokens::{
    CORNER_CONTROL, FS_BODY, GLOW_PRIMARY, SPACE_3, TOUCH_TARGET, Weight, font,
};

const FIELD_WIDTH: f32 = 2.0 * TOUCH_TARGET;

/// Shows a whole-number field drawn to the theme's input contract, which also covers
/// `input[type=number]`: `control-bg` fill, a `control-border` border that turns
/// `color-primary` on hover and focus, and the `glow-primary` bloom while focused. Dragging
/// or typing changes `value` within `range`.
pub(crate) fn number_field(
    ui: &mut Ui,
    value: &mut u32,
    range: RangeInclusive<u32>,
    enabled: bool,
) -> Response {
    let glow = ui.painter().add(Shape::Noop);
    let response = ui
        .scope(|ui| {
            let style = ui.style_mut();
            style.override_font_id = Some(font(FS_BODY, Weight::Regular));
            style.spacing.interact_size = vec2(FIELD_WIDTH, TOUCH_TARGET);
            style.spacing.button_padding = vec2(SPACE_3, 0.0);
            let widgets = &mut style.visuals.widgets;
            for (state, border) in [
                (&mut widgets.inactive, palette::CONTROL_BORDER),
                (&mut widgets.hovered, palette::PRIMARY),
                (&mut widgets.active, palette::PRIMARY),
            ] {
                state.bg_fill = palette::CONTROL_BG;
                state.weak_bg_fill = palette::CONTROL_BG;
                state.bg_stroke = Stroke::new(1.0, border);
                state.fg_stroke = Stroke::new(1.0, palette::CONTROL_FG);
                state.corner_radius = CORNER_CONTROL;
                state.expansion = 0.0;
            }
            ui.add_enabled(enabled, DragValue::new(value).range(range))
        })
        .inner;
    if response.has_focus() {
        ui.painter()
            .set(glow, GLOW_PRIMARY.as_shape(response.rect, CORNER_CONTROL));
    }
    response
}
