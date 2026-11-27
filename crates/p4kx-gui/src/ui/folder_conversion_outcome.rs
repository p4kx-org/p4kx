// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::Duration;

use egui::{Id, Ui};
use fluent_bundle::FluentValue;
use p4kx_extraction::Progress;

use super::extraction::number;
use super::extraction_outcome::{decorate, lists, outcome_window, ratio};
use crate::theme::desktop::{MODAL_WIDTH, SPINNER_INLINE};
use crate::theme::palette;
use crate::theme::tokens::SPACE_3;
use crate::widgets::button::Kind;
use crate::widgets::frames::{self, Tone};
use crate::widgets::indicator;
use crate::widgets::placement;
use crate::{
    ExtractionStatus, ExtractionSummary, ExtractionText, FolderConversionText as T, Model, Texts,
};

const REFRESH: Duration = Duration::from_millis(100);
const WINDOW: &str = "folder-conversion-run";

/// Shows the progress of the conversion of a folder, or its outcome once it ended.
pub(super) fn show(ui: &Ui, model: &mut Model) {
    let Some(run) = model.extraction() else {
        return;
    };
    let texts = model.texts();
    let input = run.folder().display().to_string();
    let mut cancel = false;
    let mut open = true;
    match run.status() {
        ExtractionStatus::Running(progress) => {
            cancel = running_window(ui, texts, progress, run.is_cancelled());
        }
        ExtractionStatus::Finished(summary) => {
            let title = if summary.cancelled().is_empty() {
                T::SummaryTitle
            } else {
                T::SummaryCancelledTitle
            };
            outcome_window(ui, texts, (title, Tone::Normal), &mut open, |ui| {
                finished(ui, texts, summary, &input);
            });
        }
        ExtractionStatus::Failed(reason) => {
            let text = match reason {
                Some(reason) => texts.format(
                    T::Failed,
                    &[("path", input.into()), ("reason", reason.to_owned().into())],
                ),
                None => texts.format(T::Stopped, &[("path", input.into())]),
            };
            outcome_window(ui, texts, (T::FailedTitle, Tone::Danger), &mut open, |ui| {
                ui.colored_label(palette::DANGER_TEXT, text);
            });
        }
    }
    if cancel {
        model.cancel_extraction();
    }
    if !open {
        model.close_extraction();
    }
}

/// Shows the non-modal progress window; returns whether Cancel was pressed.
fn running_window(ui: &Ui, texts: &Texts, progress: Progress, cancelling: bool) -> bool {
    let mut cancel = false;
    let title = texts.get(T::RunningTitle);
    let shown = placement::window(ui.ctx(), &title, Tone::Normal)
        .id(Id::unique(WINDOW))
        .collapsible(false)
        .resizable(false)
        .default_width(MODAL_WIDTH)
        .show(ui.ctx(), |ui| {
            frames::head(ui, (&title, Tone::Normal), false, None);
            cancel = running(ui, texts, progress, cancelling);
        });
    decorate(ui, shown, Tone::Normal);
    ui.ctx().request_repaint_after(REFRESH);
    cancel
}

fn running(ui: &mut Ui, texts: &Texts, progress: Progress, cancelling: bool) -> bool {
    if progress.entries_total == 0 {
        ui.horizontal(|ui| {
            indicator::spinner(ui, SPINNER_INLINE);
            ui.label(texts.get(T::Scanning));
        });
        ui.add_space(SPACE_3);
    } else {
        let fraction = if progress.bytes_total == 0 {
            1.0
        } else {
            ratio(progress.bytes_done, progress.bytes_total)
        };
        let percent = (fraction * 100.0).floor();
        ui.label(texts.format(
            T::Progress,
            &[
                ("done", number(texts, progress.entries_done)),
                ("total", number(texts, progress.entries_total)),
                ("percent", FluentValue::from(f64::from(percent))),
            ],
        ));
        ui.add_space(SPACE_3);
        let width = ui.available_width().max(frames::MODAL_BODY_WIDTH);
        indicator::progress(ui, fraction, cancelling, width);
    }
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

fn finished(ui: &mut Ui, texts: &Texts, summary: &ExtractionSummary, input: &str) {
    let counts = [
        ("extracted", number(texts, summary.extracted())),
        (
            "converted",
            number(texts, summary.converted().unwrap_or_default()),
        ),
        ("skipped", number(texts, summary.skipped().len())),
        ("failed", number(texts, summary.failures().len())),
        ("cancelled", number(texts, summary.cancelled().len())),
        ("left", number(texts, summary.not_converted())),
    ];
    ui.label(texts.format(T::SummaryCounts, &counts));
    super::extraction_record_lists::counts(ui, texts, summary);
    ui.label(texts.format(T::SummaryInput, &[("path", input.to_owned().into())]));
    let target = summary.target().display().to_string();
    ui.label(texts.format(ExtractionText::SummaryTarget, &[("path", target.into())]));
    lists(ui, texts, summary);
}
