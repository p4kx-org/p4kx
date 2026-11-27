// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use super::{list, list_row, list_rows, number};
use crate::{ExtractionText, RecordExportSummary, RecordExportText, Texts};

pub(super) fn show(ui: &mut Ui, texts: &Texts, summary: &RecordExportSummary) {
    let receipts = summary.origin_receipts();
    if !receipts.is_empty() {
        let label = texts.format(
            ExtractionText::SummaryFileOriginReceipts,
            &[("number", number(texts, receipts.len()))],
        );
        list_rows(
            ui,
            (&label, "record origin receipts"),
            receipts.len(),
            false,
            |ui, index| {
                if let Some(path) = receipts.get(index) {
                    let relative = summary
                        .target()
                        .and_then(|target| path.strip_prefix(target).ok())
                        .unwrap_or(path);
                    list_row(ui, &relative.to_string_lossy(), None);
                }
            },
        );
    }
    let failures = summary.origin_failures();
    if !failures.is_empty() {
        let label = texts.format(
            RecordExportText::SummaryOriginFailures,
            &[("number", number(texts, failures.len()))],
        );
        list(ui, (&label, "record origin failures"), failures, None, true);
    }
}
