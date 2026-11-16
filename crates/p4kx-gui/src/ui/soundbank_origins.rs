// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Response;

use super::source_origin;
use crate::widgets::tooltip;
use crate::{AudioText, ComparisonText, SoundbankPreview, Text, Texts};

pub(super) fn summary(response: Response, texts: &Texts, bank: &SoundbankPreview) {
    let Some(summary) = bank.summary_provenance() else {
        return;
    };
    let origins = summary.ok();
    tooltip::hover(response, |ui| {
        for (field, origin) in [
            (
                Text::from(ComparisonText::ColumnPath),
                origins.map(|proof| &proof.path),
            ),
            (Text::ColumnSize, origins.map(|proof| &proof.size)),
            (
                AudioText::BankWemCount.into(),
                origins.map(|proof| &proof.wem_count),
            ),
        ] {
            source_origin::row(ui, texts, field, origin);
        }
    });
}

pub(super) fn row(response: Response, texts: &Texts, bank: &SoundbankPreview, id: u32) {
    tooltip::hover(response, |ui| {
        let origins = bank.row_provenance(id).and_then(Result::ok);
        for (field, origin) in [
            (
                Text::from(AudioText::ColumnId),
                origins.map(|proof| &proof.identity.id),
            ),
            (AudioText::Codec.into(), origins.map(|proof| &proof.codec)),
            (
                AudioText::Duration.into(),
                origins.map(|proof| &proof.duration),
            ),
            (Text::ColumnSize, origins.map(|proof| &proof.identity.size)),
        ] {
            source_origin::row(ui, texts, field, origin);
        }
    });
}
