// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::Duration;

use egui::{Id, Ui};
use fluent_bundle::FluentValue;
use p4kx_assembly::audio::ExportReport;

use super::extraction::number;
use super::extraction_outcome::{decorate, ratio};
use super::record_export_outcome::{list, outcome_window};
use crate::theme::desktop::{MODAL_WIDTH, SPINNER_INLINE};
use crate::theme::palette;
use crate::theme::tokens::SPACE_3;
use crate::widgets::button::Kind;
use crate::widgets::frames::{self, Tone};
use crate::widgets::indicator;
use crate::widgets::placement;
use crate::{
    ExtractionText, Model, SoundsExport, SoundsExportStatus, SoundsText as T, Text, Texts,
};

const REFRESH: Duration = Duration::from_millis(100);

/// Shows the progress of a running sound export, or its summary once it ended.
#[expect(clippy::too_many_lines, reason = "one arm per state of the export")]
pub(super) fn show(ui: &Ui, model: &mut Model) {
    let Some(export) = model.sounds().export() else {
        return;
    };
    let texts = model.texts();
    let mut cancel = false;
    let mut close = false;
    match export.status() {
        SoundsExportStatus::Running {
            done,
            total,
            cancelling,
        } => {
            let title = texts.get(T::ExportRunning);
            let shown = placement::window(ui.ctx(), &title, Tone::Normal)
                .id(Id::unique("sounds-export-run"))
                .collapsible(false)
                .resizable(false)
                .default_width(MODAL_WIDTH)
                .show(ui.ctx(), |ui| {
                    frames::head(ui, (&title, Tone::Normal), false, None);
                    cancel = running(ui, texts, (done, total), cancelling);
                });
            decorate(ui, shown, Tone::Normal);
            ui.ctx().request_repaint_after(REFRESH);
        }
        SoundsExportStatus::Finished {
            report,
            undefined,
            cancelled,
        } => {
            let title = if cancelled {
                T::SummaryCancelledTitle
            } else {
                T::SummaryTitle
            };
            close = outcome_window(
                ui,
                texts,
                ("sounds-export-outcome", Text::Sounds(title), Tone::Normal),
                |ui| finished(ui, texts, export, (report, undefined, cancelled)),
            );
        }
        SoundsExportStatus::Failed(reason) => {
            let path = ("path", export.folder().display().to_string().into());
            let text = match reason {
                Some(reason) => texts.format(
                    T::ExportFailed,
                    &[path, ("reason", reason.to_owned().into())],
                ),
                None => texts.format(T::ExportStopped, &[path]),
            };
            close = outcome_window(
                ui,
                texts,
                ("sounds-export-outcome", T::FailedTitle.into(), Tone::Danger),
                |ui| {
                    ui.colored_label(palette::DANGER_TEXT, text);
                },
            );
        }
    }
    if cancel {
        model.sounds().cancel_export();
    }
    if close {
        model.sounds_mut().close_export();
    }
}

fn running(ui: &mut Ui, texts: &Texts, (done, total): (usize, usize), cancelling: bool) -> bool {
    let fraction = if total > 0 {
        ratio(done.min(total) as u64, total as u64)
    } else {
        0.0
    };
    if total == 0 {
        ui.horizontal(|ui| {
            indicator::spinner(ui, SPINNER_INLINE);
            ui.label(texts.get(T::ExportPlanning));
        });
    } else {
        let percent = (fraction * 100.0).floor();
        ui.label(texts.format(
            T::ExportProgress,
            &[
                ("done", number(texts, done.min(total))),
                ("total", number(texts, total)),
                ("percent", FluentValue::from(f64::from(percent))),
            ],
        ));
    }
    ui.add_space(SPACE_3);
    let width = ui.available_width().max(frames::MODAL_BODY_WIDTH);
    indicator::progress(ui, fraction, cancelling, width);
    frames::foot_rule(ui);
    if cancelling {
        ui.horizontal(|ui| {
            indicator::spinner(ui, SPINNER_INLINE);
            ui.label(texts.get(ExtractionText::Cancelling));
        });
        false
    } else {
        frames::foot_action(ui, &texts.get(ExtractionText::Cancel), Kind::Ghost)
    }
}

#[expect(clippy::too_many_lines, reason = "one list per kind of outcome")]
fn finished(
    ui: &mut Ui,
    texts: &Texts,
    export: &SoundsExport,
    (report, undefined, cancelled): (&ExportReport, &[String], bool),
) {
    if cancelled {
        ui.colored_label(palette::WARNING, texts.get(T::SummaryCancelled));
    }
    ui.label(texts.format(
        T::SummaryCounts,
        &[
            ("written", number(texts, report.written_count())),
            ("skipped", number(texts, report.kept_count())),
            (
                "failed",
                number(texts, report.failures.len() + undefined.len()),
            ),
        ],
    ));
    ui.label(texts.format(
        T::SummaryTarget,
        &[("path", export.folder().display().to_string().into())],
    ));
    let reason = texts.get(T::Undefined);
    let mut failures: Vec<String> = report
        .failures
        .iter()
        .map(|failure| {
            let mut args = vec![
                ("name", failure.trigger.clone().into()),
                ("reason", failure.reason.clone().into()),
            ];
            let text = failure.wem_id.map_or(T::SummaryFailurePlain, |id| {
                args.push(("id", id.to_string().into()));
                T::SummaryFailure
            });
            texts.format(text, &args)
        })
        .collect();
    failures.extend(undefined.iter().map(|name| {
        texts.format(
            T::SummaryFailurePlain,
            &[
                ("name", name.clone().into()),
                ("reason", reason.clone().into()),
            ],
        )
    }));
    let file_note = |path: &Path, reason: &str| {
        let name = path.file_name()?.to_string_lossy().into_owned();
        Some(texts.format(
            T::SummaryFile,
            &[("name", name.into()), ("reason", reason.to_owned().into())],
        ))
    };
    let unconverted: Vec<_> = report
        .sounds
        .iter()
        .filter_map(|sound| file_note(&sound.path, sound.unconverted.as_deref()?))
        .collect();
    let receipt_failures: Vec<_> = report
        .sounds
        .iter()
        .filter_map(|sound| file_note(&sound.path, sound.origin_receipt_failure.as_deref()?))
        .collect();
    let heading = |text: T, len: usize| {
        texts.format(
            text,
            &[
                (
                    "count",
                    FluentValue::from(u64::try_from(len).unwrap_or(u64::MAX)),
                ),
                ("number", number(texts, len)),
            ],
        )
    };
    list(
        ui,
        (&heading(T::SummaryFailed, failures.len()), "sounds-failed"),
        &failures,
        None,
        true,
    );
    list(
        ui,
        (
            &heading(T::SummaryReceiptFailed, receipt_failures.len()),
            "sounds-receipts-failed",
        ),
        &receipt_failures,
        None,
        true,
    );
    list(
        ui,
        (
            &heading(T::SummaryUnconverted, unconverted.len()),
            "sounds-unconverted",
        ),
        &unconverted,
        None,
        false,
    );
}
