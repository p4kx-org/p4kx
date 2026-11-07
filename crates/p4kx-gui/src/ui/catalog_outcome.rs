// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::time::Duration;

use egui::{Id, Ui};
use fluent_bundle::FluentValue;
use p4kx_export::{
    CatalogPublicationMode, CatalogSummary, DECOMPRESSED_LIMIT, SizeWarning, UPLOAD_LIMIT,
};

use super::catalog_lines::{Notice, failure_lines, kinds, languages, notices, playability, sizes};
use super::extraction::{group_label, number};
use super::extraction_outcome::{decorate, ratio};
use super::record_export_outcome::{list, outcome_window};
use crate::theme::desktop::{MODAL_GROUP_GAP, MODAL_WIDTH, SPINNER_INLINE};
use crate::theme::palette;
use crate::theme::tokens::SPACE_3;
use crate::widgets::button::Kind;
use crate::widgets::frames::{self, Tone};
use crate::widgets::indicator;
use crate::widgets::placement;
use crate::{
    CatalogExport, CatalogExportProgress, CatalogExportStage, CatalogExportStatus,
    CatalogExportSummary, CatalogText as T, ExtractionText, Language, Model, Texts, format,
};

const REFRESH: Duration = Duration::from_millis(100);
const WINDOW: &str = "catalog-run";
const OUTCOME: &str = "catalog-outcome";

/// Shows the progress of a running Basetool catalog, or its outcome once it ended.
pub(super) fn show(ui: &Ui, model: &mut Model) {
    let Some(run) = model.catalog() else {
        return;
    };
    let texts = model.texts();
    let mut cancel = false;
    let mut close = false;
    match run.status() {
        CatalogExportStatus::Running(progress) => {
            cancel = running_window(ui, texts, run, progress);
        }
        CatalogExportStatus::Finished(summary) => {
            close = finished_window(ui, texts, run, summary);
        }
        CatalogExportStatus::Cancelled => {
            close = outcome_window(
                ui,
                texts,
                (OUTCOME, T::CancelledTitle.into(), Tone::Normal),
                |ui| {
                    ui.colored_label(palette::WARNING, texts.get(T::Cancelled));
                },
            );
        }
        CatalogExportStatus::IsArchive => {
            let text = texts.format(T::IsArchive, &[("path", run.shown_file().into())]);
            close = failure_window(ui, texts, text);
        }
        CatalogExportStatus::GzipNotSupported => {
            let text = texts.format(T::GzipNotSupported, &[("path", run.shown_file().into())]);
            close = failure_window(ui, texts, text);
        }
        CatalogExportStatus::Incomplete(_) => {
            close = failure_window(ui, texts, texts.get(T::Incomplete));
        }
        CatalogExportStatus::Failed(reason) => {
            let path = ("path", run.shown_file().into());
            let text = match reason {
                Some(reason) => texts.format(T::Failed, &[path, ("reason", reason.into())]),
                None => texts.format(T::Stopped, &[path]),
            };
            close = failure_window(ui, texts, text);
        }
    }
    if cancel {
        model.cancel_catalog();
    }
    if close {
        model.close_catalog();
    }
}

fn finished_window(
    ui: &Ui,
    texts: &Texts,
    run: &CatalogExport,
    summary: &CatalogExportSummary,
) -> bool {
    let diagnostic = run.publication_mode() == CatalogPublicationMode::DiagnosticPartial;
    let title = if diagnostic {
        T::DiagnosticSummaryTitle
    } else {
        T::SummaryTitle
    };
    let laid = Laid::get(ui, texts, (run.id(), summary));
    outcome_window(ui, texts, (OUTCOME, title.into(), Tone::Normal), |ui| {
        if diagnostic {
            ui.colored_label(palette::WARNING, texts.get(T::DiagnosticNotice));
            ui.add_space(SPACE_3);
        }
        if summary.summary().export_health().is_complete() {
            ui.label(texts.get(T::HealthComplete));
        } else {
            ui.colored_label(palette::WARNING, texts.get(T::HealthIncomplete));
        }
        ui.add_space(SPACE_3);
        finished(ui, &laid);
    })
}

/// Shows the danger window of a catalog that wrote nothing, saying why; returns whether it was
/// closed.
fn failure_window(ui: &Ui, texts: &Texts, text: String) -> bool {
    outcome_window(
        ui,
        texts,
        (OUTCOME, T::FailedTitle.into(), Tone::Danger),
        |ui| {
            ui.colored_label(palette::DANGER_TEXT, text);
        },
    )
}

/// Shows the non-modal progress window; returns whether Cancel was pressed.
fn running_window(
    ui: &Ui,
    texts: &Texts,
    run: &CatalogExport,
    progress: CatalogExportProgress,
) -> bool {
    let mut cancel = false;
    let cancelling = run.is_cancelled();
    let title = texts.get(
        if run.publication_mode() == CatalogPublicationMode::DiagnosticPartial {
            T::DiagnosticRunningTitle
        } else {
            T::RunningTitle
        },
    );
    let path = run.shown_file();
    let shown = placement::window(ui.ctx(), &title, Tone::Normal)
        .id(Id::unique(WINDOW))
        .collapsible(false)
        .resizable(false)
        .default_width(MODAL_WIDTH)
        .show(ui.ctx(), |ui| {
            frames::head(ui, (&title, Tone::Normal), false, None);
            cancel = running(ui, texts, (progress, path), cancelling);
        });
    decorate(ui, shown, Tone::Normal);
    ui.ctx().request_repaint_after(REFRESH);
    cancel
}

fn running(
    ui: &mut Ui,
    texts: &Texts,
    (progress, path): (CatalogExportProgress, &str),
    cancelling: bool,
) -> bool {
    match progress.stage {
        CatalogExportStage::Reading => {
            waiting(ui, &texts.get(T::Reading));
            ui.add_space(SPACE_3);
        }
        CatalogExportStage::Building if progress.total == 0 => {
            waiting(ui, &texts.get(T::Reading));
            ui.add_space(SPACE_3);
        }
        CatalogExportStage::Writing => {
            waiting(ui, &texts.format(T::Writing, &[("path", path.into())]));
            ui.add_space(SPACE_3);
        }
        CatalogExportStage::Building => {
            let fraction = ratio(progress.done as u64, progress.total as u64);
            let percent = (fraction * 100.0).floor();
            let line = if progress.crafting {
                T::CraftingProgress
            } else {
                T::Progress
            };
            ui.label(texts.format(
                line,
                &[
                    ("done", number(texts, progress.done.min(progress.total))),
                    ("total", number(texts, progress.total)),
                    ("percent", FluentValue::from(f64::from(percent))),
                ],
            ));
            ui.add_space(SPACE_3);
            let width = ui.available_width().max(frames::MODAL_BODY_WIDTH);
            indicator::progress(ui, fraction, cancelling, width);
        }
    }
    frames::foot_rule(ui);
    if cancelling {
        waiting(ui, &texts.get(ExtractionText::Cancelling));
        false
    } else {
        frames::foot_action(ui, &texts.get(ExtractionText::Cancel), Kind::Ghost)
    }
}

fn waiting(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        indicator::spinner(ui, SPINNER_INLINE);
        ui.label(text);
    });
}

/// Returns the texts of the near and over warning of the upload size; a plain JSON catalog also
/// advises `.json.zst`.
const fn size_texts(compressed: bool) -> [T; 2] {
    if compressed {
        [T::NearLimit, T::OverLimit]
    } else {
        [T::NearLimitPlain, T::OverLimitPlain]
    }
}

/// Returns the sentence of every size warning of `summary`, with the level it has.
fn size_warnings(
    texts: &Texts,
    summary: &CatalogSummary,
    limit: &str,
) -> Vec<(String, SizeWarning)> {
    let decompressed = format::decimal_size(DECOMPRESSED_LIMIT, texts.language());
    [
        (
            summary.size_warning(),
            size_texts(summary.compressed_size().is_some()),
            limit,
        ),
        (
            summary.decompressed_warning(),
            [T::DecompressedNearLimit, T::DecompressedOverLimit],
            decompressed.as_str(),
        ),
    ]
    .into_iter()
    .filter_map(|(warning, [near, over], shown)| {
        let warning = warning?;
        let text = match warning {
            SizeWarning::NearLimit => near,
            SizeWarning::OverLimit => over,
        };
        Some((texts.format(text, &[("limit", shown.into())]), warning))
    })
    .collect()
}

/// The texts of one summary, formatted once per catalog and language.
struct Laid {
    key: (u64, Language),
    kinds_heading: String,
    playability_heading: String,
    texts_heading: String,
    warnings: Vec<(String, SizeWarning)>,
    notices: Vec<Notice>,
    kinds: Vec<String>,
    skipped: String,
    playability: Vec<String>,
    failed_heading: String,
    failures: Vec<String>,
    reasons: Vec<usize>,
    texts: Vec<String>,
    missing: String,
    size: String,
    file: String,
}

impl Laid {
    fn get(ui: &Ui, texts: &Texts, (run, summary): (u64, &CatalogExportSummary)) -> Arc<Self> {
        let key = (run, texts.language());
        let id = Id::unique("p4kx-catalog-summary");
        if let Some(cached) = ui.data(|data| data.get_temp::<Arc<Self>>(id))
            && cached.key == key
        {
            return cached;
        }
        let laid = Arc::new(Self::new(key, texts, summary));
        ui.data_mut(|data| data.insert_temp(id, Arc::clone(&laid)));
        laid
    }

    fn new(key: (u64, Language), texts: &Texts, outcome: &CatalogExportSummary) -> Self {
        let summary = outcome.summary();
        let limit = format::decimal_size(UPLOAD_LIMIT, texts.language());
        let warnings = size_warnings(texts, summary, &limit);
        let (failures, reasons) = failure_lines(texts, summary);
        Self {
            key,
            kinds_heading: texts.get(T::KindsHeading),
            playability_heading: texts.get(T::PlayabilityHeading),
            texts_heading: texts.get(T::TextsHeading),
            warnings,
            notices: notices(texts, summary),
            kinds: kinds(texts, summary),
            skipped: texts.format(
                T::SummarySkipped,
                &[("number", number(texts, summary.skipped()))],
            ),
            failed_heading: texts.format(
                T::SummaryFailed,
                &[("number", number(texts, summary.failures().len()))],
            ),
            failures,
            reasons,
            playability: playability(texts, summary),
            texts: languages(texts, summary),
            missing: texts.format(
                T::SummaryMissing,
                &[("number", number(texts, summary.unresolved()))],
            ),
            size: sizes(texts, summary, limit),
            file: texts.format(
                T::SummaryFile,
                &[("path", outcome.file().display().to_string().into())],
            ),
        }
    }
}

fn finished(ui: &mut Ui, laid: &Laid) {
    for (warning, level) in &laid.warnings {
        let body = |ui: &mut Ui| {
            ui.colored_label(palette::GRAY_1, warning);
        };
        match level {
            SizeWarning::NearLimit => frames::warning_alert(ui, body),
            SizeWarning::OverLimit => frames::danger_alert(ui, body),
        }
        ui.add_space(MODAL_GROUP_GAP);
    }
    for notice in &laid.notices {
        frames::warning_alert(ui, |ui| {
            ui.colored_label(palette::GRAY_1, &notice.text);
            for detail in &notice.details {
                ui.colored_label(palette::GRAY_2_TEXT, detail);
            }
        });
        ui.add_space(MODAL_GROUP_GAP);
    }
    group_label(ui, &laid.kinds_heading);
    lines(ui, &laid.kinds);
    note(ui, &laid.skipped);
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &laid.playability_heading);
    lines(ui, &laid.playability);
    list(
        ui,
        (&laid.failed_heading, "catalog-failed"),
        &laid.failures,
        Some(&laid.reasons),
        true,
    );
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &laid.texts_heading);
    lines(ui, &laid.texts);
    note(ui, &laid.missing);
    ui.add_space(MODAL_GROUP_GAP);
    note(ui, &laid.size);
    note(ui, &laid.file);
}

fn lines(ui: &mut Ui, lines: &[String]) {
    for line in lines {
        note(ui, line);
    }
}

fn note(ui: &mut Ui, text: &str) {
    ui.colored_label(palette::GRAY_1, text);
}
