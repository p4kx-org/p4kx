// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align2, Frame, Margin, TextEdit, Ui, vec2};

use super::extraction::group_label;
use crate::theme::palette;
use crate::theme::tokens::{FS_BODY, FS_XS, SPACE_3, TOUCH_TARGET, Weight, font};
use crate::widgets::field;
use crate::widgets::frames::px;
use crate::widgets::text::plain;
use crate::{AssemblyExportSetup, AssemblyExportText as T, Texts};

/// The most characters of the picks field.
const MAX_CHARACTERS: usize = 512;

/// Shows the field of the Loadout picks: the alternatives of the groups of a generated Loadout,
/// with a hint of the form and an error note while the text is no picks.
pub(super) fn picks(ui: &mut Ui, texts: &Texts, setup: &mut AssemblyExportSetup) {
    group_label(ui, &texts.get(T::PicksHeading));
    let mut text = setup.picks_text().to_owned();
    let bloom = field::bloom_slot(ui);
    let frame = field::frame_slot(ui);
    let edit = TextEdit::singleline(&mut text)
        .char_limit(MAX_CHARACTERS)
        .hint_text(plain(texts.get(T::PicksHint), FS_BODY, Weight::Regular).italics())
        .font(font(FS_BODY, Weight::Regular))
        .frame(Frame::NONE.inner_margin(Margin::symmetric(px(SPACE_3), 0)))
        .min_size(vec2(0.0, TOUCH_TARGET))
        .desired_width(ui.available_width())
        .align(Align2::LEFT_CENTER);
    let response = ui.add(edit);
    if setup.picks_invalid() {
        field::paint_error_frame(ui, frame, response.rect);
    } else {
        field::paint_frame(ui, frame, response.rect, response.has_focus());
    }
    if response.has_focus() {
        field::paint_bloom(ui, bloom, response.rect);
    }
    if response.changed() {
        setup.set_picks_text(text);
    }
    if setup.picks_invalid() {
        field::error_note(ui, &texts.get(T::PicksInvalid));
    } else {
        let note = plain(texts.get(T::PicksNote), FS_XS, Weight::Regular);
        ui.add(egui::Label::new(note.color(palette::GRAY_2_TEXT)).wrap());
    }
}
