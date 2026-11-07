// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::output_target::PickerFailure;
use crate::theme::tokens::{FS_SM, Weight};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames::{self, Tone};
use crate::widgets::placement;
use crate::widgets::text::plain;
use crate::{ExtractionText, OutputTargetText as T, Text, Texts};

pub(crate) fn show(ui: &egui::Ui, texts: &Texts, failure: &mut Option<PickerFailure>) {
    let Some(reason) = failure.as_ref() else {
        return;
    };
    let title = texts.get(T::PickerFailureTitle);
    let close = texts.get(Text::CloseDialog);
    let dismiss = texts.get(ExtractionText::Close);
    let shown = placement::hanging_modal(
        ui.ctx(),
        frames::modal_id("picker-failure"),
        Tone::Danger,
    )
    .show(ui.ctx(), |ui| {
        ui.set_width(frames::MODAL_WIDE_BODY_WIDTH);
        let mut closed = frames::head(ui, (&title, Tone::Danger), true, Some(&close));
        let room = placement::body_room(ui);
        frames::scrolling_body_in(ui, ("picker-failure-body", room), |ui| {
            frames::danger_alert(ui, |ui| {
                ui.add(
                    egui::Label::new(plain(reason.message(texts), FS_SM, Weight::Regular)).wrap(),
                );
            });
        });
        frames::foot_rule(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            closed |= button::button(ui, Kind::Cta, Size::Normal, &dismiss, true).clicked();
        });
        closed
    });
    frames::modal_decoration(
        ui.ctx(),
        shown.response.layer_id,
        shown.response.rect,
        Tone::Danger,
    );
    if shown.inner || shown.should_close() {
        *failure = None;
    }
}
