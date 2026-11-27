// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::gfx::{Skipped, TagFailure};
use p4kx_extraction::{ImportReport, UiBitmapFailure};
use p4kx_screens::{ContentOmission, Still};

use crate::UiText;
use crate::model::describe;

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;

/// One thing a render left out or could not do: what kind of thing it is, and the details the
/// renderer gives, which are the words `p4kx ui render` prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportLine {
    /// The label that names the kind of thing.
    pub label: UiText,
    /// The details.
    pub detail: String,
}

/// What a render left out, as `p4kx ui render` reports it, and how much of a screen it drew.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiReport {
    lines: Vec<ReportLine>,
    counts: Vec<(UiText, usize)>,
    content_omissions: Vec<ContentOmission>,
}

impl UiReport {
    /// Returns the grouped diagnostics in the order of the renderer's report.
    #[must_use]
    pub fn lines(&self) -> &[ReportLine] {
        &self.lines
    }

    /// Returns the number of grouped diagnostic lines.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns whether there are any grouped diagnostic lines.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Returns the content omissions with their original widget and source fields.
    #[must_use]
    pub fn content_omissions(&self) -> &[ContentOmission] {
        &self.content_omissions
    }

    /// Returns the counts of a screen: its widgets, the shapes drawn, and the texts drawn and
    /// left out; empty for a movie.
    #[must_use]
    pub fn counts(&self) -> &[(UiText, usize)] {
        &self.counts
    }

    pub(super) fn warned(mut self, warnings: &[String]) -> Self {
        let mut lines: Vec<ReportLine> = warnings
            .iter()
            .map(|warning| ReportLine {
                label: UiText::ReportWarning,
                detail: warning.clone(),
            })
            .collect();
        lines.append(&mut self.lines);
        self.lines = lines;
        self
    }

    pub(super) fn of_movie(
        skipped: &[Skipped],
        bitmaps: &[UiBitmapFailure],
        tags: &[TagFailure],
        imports: &ImportReport,
    ) -> Self {
        let mut lines = Vec::new();
        for skipped in skipped {
            let place = skipped.sprite.map_or_else(
                || format!("depth {}", skipped.depth),
                |sprite| format!("depth {} of sprite {sprite}", skipped.depth),
            );
            lines.push(ReportLine {
                label: if skipped.reason.is_effect() {
                    UiText::ReportEffect
                } else {
                    UiText::ReportSkipped
                },
                detail: format!("definition {} at {place}: {}", skipped.id, skipped.reason),
            });
        }
        for import in &imports.unresolved {
            lines.push(ReportLine {
                label: UiText::ReportImport,
                detail: format!("{} from {}: {}", import.name, import.url, import.issue),
            });
        }
        for failure in tags {
            lines.push(ReportLine {
                label: UiText::ReportTag,
                detail: format!(
                    "tag {} at offset {}: {}",
                    failure.code,
                    failure.offset,
                    describe(&failure.error)
                ),
            });
        }
        for failure in bitmaps {
            lines.push(ReportLine {
                label: UiText::ReportBitmap,
                detail: format!("bitmap {}: {}", failure.id, describe(&failure.error)),
            });
        }
        Self {
            lines,
            counts: Vec::new(),
            content_omissions: Vec::new(),
        }
    }

    pub(super) fn of_still(still: &Still, loading: &[String]) -> Self {
        let report = still.report();
        let mut lines: Vec<ReportLine> = loading
            .iter()
            .map(|warning| ReportLine {
                label: UiText::ReportWarning,
                detail: warning.clone(),
            })
            .collect();
        lines.extend(still.skipped().iter().map(|skipped| ReportLine {
            label: UiText::ReportSkipped,
            detail: skipped.reason.to_string(),
        }));
        lines.extend(report.unsupported.iter().map(|entry| ReportLine {
            label: UiText::ReportLeftOut,
            detail: format!(
                "{} x{}: {} ({})",
                entry.element,
                entry.count,
                entry.reason,
                entry.examples.join(", ")
            ),
        }));
        lines.extend(report.unknown_variables().map(|name| ReportLine {
            label: UiText::ReportUnknownVariable,
            detail: name.to_owned(),
        }));
        lines.extend(report.missing_canvases.iter().map(|canvas| ReportLine {
            label: UiText::ReportMissingCanvas,
            detail: canvas.clone(),
        }));
        lines.extend(report.missing_textures.iter().map(|texture| ReportLine {
            label: UiText::ReportMissingTexture,
            detail: texture.clone(),
        }));
        if report.truncated {
            lines.push(ReportLine {
                label: UiText::ReportTruncated,
                detail: String::new(),
            });
        }
        Self {
            lines,
            counts: vec![
                (UiText::FactWidgets, report.widgets),
                (UiText::FactShapes, report.shapes),
                (UiText::FactTextsDrawn, report.texts_drawn),
                (UiText::FactTextsLeftOut, report.texts_left_out),
            ],
            content_omissions: report.content_omissions.clone(),
        }
    }
}
