// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::time::Duration;

use egui::{Color32, Id, Ui};
use fluent_bundle::FluentValue;
use p4kx_extraction::Guid;

use super::extraction_outcome::{decorate, ratio};
use super::model_export_animations as animations;
use super::model_export_notes::{missing_textures, notes, number};
use super::model_info_rows::file_name;
use super::record_export_outcome::{list, outcome_window};
use crate::theme::desktop::{MODAL_WIDTH, SPINNER_INLINE};
use crate::theme::palette;
use crate::theme::tokens::SPACE_3;
use crate::widgets::button::Kind;
use crate::widgets::frames::{self, Tone};
use crate::widgets::indicator;
use crate::widgets::placement;
use crate::{
    ExtractionText, Language, Model, ModelExportFailure, ModelExportProgress, ModelExportStage,
    ModelExportStatus, ModelExportSummary, ModelExportText as T, ModelInfoText, Texts, format,
};

const REFRESH: Duration = Duration::from_millis(100);
const WINDOW: &str = "model-export-run";
const OUTCOME: &str = "model-export-outcome";

/// Shows the progress of a running Export as GLB, or its outcome once it ended.
pub(super) fn show(ui: &Ui, model: &mut Model) {
    let Some(run) = model.model_export() else {
        return;
    };
    let texts = model.texts();
    let name = || ("name", run.name().to_owned().into());
    let mut cancel = false;
    let mut close = false;
    match run.status() {
        ModelExportStatus::Running(progress) => {
            let cancelling = run.is_cancelled();
            let title = texts.get(T::RunningTitle);
            let shown = placement::window(ui.ctx(), &title, Tone::Normal)
                .id(Id::unique(WINDOW))
                .collapsible(false)
                .resizable(false)
                .default_width(MODAL_WIDTH)
                .show(ui.ctx(), |ui| {
                    frames::head(ui, (&title, Tone::Normal), false, None);
                    cancel = running(ui, texts, (run.name(), progress), cancelling);
                });
            decorate(ui, shown, Tone::Normal);
            ui.ctx().request_repaint_after(REFRESH);
        }
        ModelExportStatus::Finished(summary) => {
            close = summary_window(ui, texts, (run.id(), summary));
        }
        ModelExportStatus::Cancelled => {
            let title = (OUTCOME, T::SummaryCancelledTitle.into(), Tone::Normal);
            close = outcome_window(ui, texts, title, |ui| {
                ui.colored_label(palette::WARNING, texts.get(T::SummaryCancelled));
            });
        }
        ModelExportStatus::Failed(failure) => {
            let (text, colour, tone) = failed(texts, failure, name());
            close = outcome_window(ui, texts, (OUTCOME, T::FailedTitle.into(), tone), |ui| {
                ui.colored_label(colour, text);
            });
        }
    }
    if cancel {
        model.cancel_model_export();
    }
    if close {
        model.close_model_export();
    }
}

pub(super) fn no_record(texts: &Texts, guid: Guid) -> String {
    texts.format(T::NoRecord, &[("guid", guid.to_string().into())])
}

fn failed(
    texts: &Texts,
    failure: &ModelExportFailure,
    name: (&str, FluentValue<'static>),
) -> (String, Color32, Tone) {
    match failure {
        ModelExportFailure::KeyRequired => (
            texts.format(T::KeyRequired, &[name]),
            palette::WARNING,
            Tone::Normal,
        ),
        ModelExportFailure::KeyRejected => (
            texts.format(T::KeyRejected, &[name]),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::ArchiveChanged => (
            texts.format(T::ArchiveChanged, &[name]),
            palette::WARNING,
            Tone::Normal,
        ),
        ModelExportFailure::Refused(refused) => (
            crate::installation::message(texts, refused),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::Failed(reason) => (
            texts.format(T::Failed, &[name, ("reason", reason.clone().into())]),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::NoRecord(guid) => (
            texts.format(
                T::Failed,
                &[name, ("reason", no_record(texts, *guid).into())],
            ),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::Stopped => (
            texts.format(T::Stopped, &[name]),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
    }
}

fn running(
    ui: &mut Ui,
    texts: &Texts,
    (name, progress): (&str, ModelExportProgress),
    cancelling: bool,
) -> bool {
    let language = texts.language();
    let fraction = if progress.stage == ModelExportStage::Writing && progress.total > 0 {
        ratio(progress.done, progress.total)
    } else {
        0.0
    };
    match progress.stage {
        ModelExportStage::Reading => {
            ui.horizontal(|ui| {
                indicator::spinner(ui, SPINNER_INLINE);
                ui.label(texts.format(T::Reading, &[("name", name.to_owned().into())]));
            });
        }
        ModelExportStage::Writing => {
            let percent = (fraction * 100.0).floor();
            ui.label(texts.format(
                T::Progress,
                &[
                    ("done", format::size(progress.done, language).into()),
                    ("total", format::size(progress.total, language).into()),
                    ("percent", FluentValue::from(f64::from(percent))),
                ],
            ));
        }
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

fn summary_window(ui: &Ui, texts: &Texts, (run, summary): (u64, &ModelExportSummary)) -> bool {
    let laid = Laid::get(ui, texts, (run, summary));
    outcome_window(
        ui,
        texts,
        (OUTCOME, T::SummaryTitle.into(), Tone::Normal),
        |ui| finished(ui, &laid),
    )
}

/// The texts of one summary, formatted once per export and language.
struct Laid {
    key: (u64, Language),
    notes: Vec<(String, Color32)>,
    heading: String,
    chunks: Vec<String>,
    textures_heading: String,
    textures: Vec<String>,
    animations_heading: String,
    animations: Vec<String>,
}

impl Laid {
    fn get(ui: &Ui, texts: &Texts, (run, summary): (u64, &ModelExportSummary)) -> Arc<Self> {
        let key = (run, texts.language());
        let id = Id::unique("p4kx-model-export-summary");
        if let Some(cached) = ui.data(|data| data.get_temp::<Arc<Self>>(id))
            && cached.key == key
        {
            return cached;
        }
        let laid = Arc::new(Self::new(key, texts, summary));
        ui.data_mut(|data| data.insert_temp(id, Arc::clone(&laid)));
        laid
    }

    fn new(key: (u64, Language), texts: &Texts, summary: &ModelExportSummary) -> Self {
        let (animations_heading, animations) = animations::unread_lines(texts, summary);
        let (textures_heading, textures) = missing_textures(texts, summary);
        Self {
            key,
            notes: notes(texts, summary),
            heading: texts.format(
                T::SummaryChunks,
                &[("number", number(texts, summary.unread_total()).into())],
            ),
            textures_heading,
            textures,
            animations_heading,
            animations,
            chunks: summary
                .unread()
                .iter()
                .map(|chunk| {
                    let args = [
                        ("index", chunk.index.to_string().into()),
                        ("type_id", chunk.type_id.clone().into()),
                        ("file", file_name(texts, chunk).into()),
                        (
                            "type",
                            chunk
                                .name
                                .map_or_else(
                                    || texts.get(ModelInfoText::TypeUnknown),
                                    str::to_owned,
                                )
                                .into(),
                        ),
                    ];
                    texts.format(
                        if chunk.unknown {
                            T::SummaryChunkUnknown
                        } else {
                            T::SummaryChunkSkipped
                        },
                        &args,
                    )
                })
                .collect(),
        }
    }
}

fn finished(ui: &mut Ui, laid: &Laid) {
    for (note, colour) in &laid.notes {
        ui.colored_label(*colour, note);
    }
    list(
        ui,
        (&laid.textures_heading, "model-export-textures"),
        &laid.textures,
        None,
        true,
    );
    list(
        ui,
        (&laid.animations_heading, "model-export-animations"),
        &laid.animations,
        None,
        true,
    );
    list(
        ui,
        (&laid.heading, "model-export-chunks"),
        &laid.chunks,
        None,
        true,
    );
}

#[cfg(test)]
#[path = "model_export_outcome_render_tests.rs"]
mod render_tests;
