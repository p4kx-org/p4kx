// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{Align, Galley, Id, Label, Layout, ScrollArea, Ui};

use crate::theme::desktop::{BUTTON_XS_HEIGHT, ROW_HEIGHT};
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, SPACE_3, SPACE_4, SPACE_8, TOUCH_TARGET, Weight, font};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames::{self, Tone};
use crate::{ExtractionText, Model, Text, theme};

pub(super) fn show(ui: &Ui, model: &mut Model) {
    if !model.licence_shown() {
        return;
    }
    let title = model.texts().get(Text::FontLicence);
    let close_label = model.texts().get(ExtractionText::Close);
    let close_dialog = model.texts().get(Text::CloseDialog);
    let shown =
        frames::modal(frames::modal_id("font-licence"), Tone::Normal).show(ui.ctx(), |ui| {
            ui.set_width(frames::MODAL_WIDE_BODY_WIDTH);
            let closed = frames::head(ui, (&title, Tone::Normal), true, Some(&close_dialog));
            ScrollArea::vertical()
                .max_height(text_height(ui))
                .show(ui, |ui| {
                    ui.add(Label::new(licence_text(ui)));
                });
            frames::foot_rule(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                button::button(ui, Kind::Cta, Size::Normal, &close_label, true).clicked()
            })
            .inner
                || closed
        });
    frames::modal_decoration(
        ui.ctx(),
        shown.response.layer_id,
        shown.response.rect,
        Tone::Normal,
    );
    if shown.inner || shown.should_close() {
        model.set_licence_shown(false);
    }
}

/// Returns the height left for the text: the window less the canvas gap above and below, the head
/// and the foot of the modal.
fn text_height(ui: &Ui) -> f32 {
    const CHROME: f32 = 2.0f32.mul_add(
        SPACE_3 + SPACE_4,
        2.0f32.mul_add(SPACE_8, BUTTON_XS_HEIGHT) + TOUCH_TARGET,
    );
    (ui.ctx().content_rect().height() - CHROME).max(3.0 * ROW_HEIGHT)
}

/// Returns the licence laid out once per width and scale, so it is not copied every frame.
fn licence_text(ui: &Ui) -> Arc<Galley> {
    let width = ui.available_width();
    let key = (width.to_bits(), ui.ctx().pixels_per_point().to_bits());
    let id = Id::unique("p4kx-font-licence");
    if let Some((cached, galley)) = ui.data(|data| data.get_temp::<((u32, u32), Arc<Galley>)>(id))
        && cached == key
    {
        return galley;
    }
    let galley = ui.fonts_mut(|fonts| {
        fonts.layout(
            theme::FONT_LICENCE.to_owned(),
            font(FS_SM, Weight::Regular),
            palette::GRAY_1,
            width,
        )
    });
    ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&galley))));
    galley
}
