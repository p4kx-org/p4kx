// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use super::extraction_outcome::{list, record_lines};
use super::record_line::record_line;
use crate::texts::describe_rename;
use crate::{ExtractionSummary, ExtractionText as T, RecordExportText, RenameText, Texts};

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
