// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use super::extraction::number;
use super::extraction_outcome::{list, record_lines};
use super::record_line::record_line;
use super::source_origin;
use crate::texts::describe_rename;
use crate::theme::palette;
use crate::widgets::tooltip;
use crate::{ExtractionSummary, ExtractionText as T, RecordExportText, RenameText, Texts};

/// Shows converted Record outcomes and the origin of the written count.
pub(super) fn counts(ui: &mut Ui, texts: &Texts, summary: &ExtractionSummary) {
    let Some(records) = summary.records() else {
        return;
    };
    let counts = [
        ("written", number(texts, records.written)),
        ("skipped", number(texts, records.skipped)),
        ("failed", number(texts, records.failed)),
        ("cancelled", number(texts, records.cancelled)),
        ("errors", number(texts, records.error_values)),
        ("erroneous", number(texts, records.records_with_errors)),
    ];
    let response = ui.label(texts.format(T::SummaryRecords, &counts));
    match summary.record_written_origin() {
        Some(Ok(proof)) => {
            tooltip::hover(response, |ui| {
                source_origin::row(
                    ui,
                    texts,
                    RecordExportText::SummaryWritten,
                    Some(&proof.written),
                );
            });
        }
        Some(Err(error)) => {
            let text = texts.format(
                T::SummaryRecordCountOriginFailed,
                &[("reason", error.to_string().into())],
            );
            ui.colored_label(palette::WARNING, text);
        }
        None => {}
    }
}

/// Shows Record payload failures, renames and companion publication failures.
pub(super) fn show(ui: &mut Ui, texts: &Texts, summary: &ExtractionSummary) {
    let heading = T::SummaryRecordsFailed;
    let lines = record_lines(
        ui,
        texts,
        (summary, heading),
        summary.record_failures(),
        |failure| {
            let line = record_line(
                texts,
                T::SummaryRecordFailure,
                &failure.record_path,
                &failure.guid,
                &failure.reason,
            );
            (line, Some(failure.reason.len()))
        },
    );
    list(ui, texts, (heading, &lines.lines, lines.reasons()), true);
    let heading = RenameText::Heading;
    let renamed = record_lines(
        ui,
        texts,
        (summary, heading),
        summary.records_renamed(),
        |note| {
            let reason = describe_rename(texts, note.cause);
            let line = record_line(
                texts,
                RenameText::Line,
                &note.record_path,
                &note.guid,
                &reason,
            );
            (line, None)
        },
    );
    list(ui, texts, (heading, &renamed.lines, None), true);
    list(
        ui,
        texts,
        (
            RecordExportText::SummaryOriginFailures,
            summary.record_origin_failures(),
            None,
        ),
        true,
    );
}
