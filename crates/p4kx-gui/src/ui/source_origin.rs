// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;
use p4kx_extraction::FieldOrigin;

use crate::widgets::tooltip;
use crate::{ImageText, Text, Texts};

pub(super) fn label(origin: Option<&FieldOrigin>) -> ImageText {
    match origin.and_then(FieldOrigin::contains_encrypted_source) {
        Some(true) => ImageText::OriginEncrypted,
        Some(false) => ImageText::OriginPlain,
        None => ImageText::OriginUnknown,
    }
}

pub(super) fn row(ui: &mut Ui, texts: &Texts, name: impl Into<Text>, origin: Option<&FieldOrigin>) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(texts.get(name));
        tooltip::line(ui, texts.get(label(origin)));
    });
}
