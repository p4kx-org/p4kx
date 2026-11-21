// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::time::Duration;

use egui::{Color32, Id, Ui};
use fluent_bundle::FluentValue;

use super::assembly_export_notes::notes;
use super::assembly_export_progress::stage_share;
use super::extraction_outcome::decorate;
use super::model_export_outcome::no_record;
use super::record_export_outcome::{list, outcome_window};
use crate::model_info::printable;
use crate::theme::desktop::{MODAL_WIDTH, SPINNER_INLINE};
use crate::theme::palette;
use crate::theme::tokens::SPACE_3;
use crate::widgets::button::Kind;
use crate::widgets::frames::{self, Tone};
use crate::widgets::indicator;
use crate::widgets::placement;
use crate::{
    AssemblyExportForm, AssemblyExportProgress, AssemblyExportStage, AssemblyExportStatus,
    AssemblyExportSummary, AssemblyExportText as T, ExtractionText, Language, Model,
    ModelExportFailure, ModelExportText, Texts, format,
};

const REFRESH: Duration = Duration::from_millis(100);
const WINDOW: &str = "assembly-export-run";
const OUTCOME: &str = "assembly-export-outcome";

/// Shows the progress of a running Export of an Entity as GLB, or its outcome once it ended.
pub(super) fn show(ui: &Ui, model: &mut Model) {
    let Some(run) = model.assembly_export() else {
        return;
    };
    let texts = model.texts();
    let name = || ("name", run.name().to_owned().into());
    let mut cancel = false;
    let mut close = false;
    match run.status() {
        AssemblyExportStatus::Running(progress) => {
            let cancelling = run.is_cancelled();
            let title = texts.get(titles(run.is_container(), run.form()).0);
            let shown = placement::window(ui.ctx(), &title, Tone::Normal)
                .id(Id::unique(WINDOW))
                .collapsible(false)
                .resizable(false)
                .default_width(MODAL_WIDTH)
                .show(ui.ctx(), |ui| {
                    frames::head(ui, (&title, Tone::Normal), false, None);
                    let package = run.form() == AssemblyExportForm::Package;
                    cancel = running(ui, texts, (run.name(), progress, package), cancelling);
                });
            decorate(ui, shown, Tone::Normal);
            ui.ctx().request_repaint_after(REFRESH);
        }
        AssemblyExportStatus::Finished(summary) => {
            let laid = Laid::get(ui, texts, (run.id(), summary));
            let title = titles(run.is_container(), run.form()).1;
            close = outcome_window(ui, texts, (OUTCOME, title.into(), Tone::Normal), |ui| {
                finished(ui, &laid);
            });
        }
        AssemblyExportStatus::Cancelled => {
            let title = (OUTCOME, T::SummaryCancelledTitle.into(), Tone::Normal);
            close = outcome_window(ui, texts, title, |ui| {
                ui.colored_label(palette::WARNING, texts.get(T::SummaryCancelled));
            });
        }
        AssemblyExportStatus::Failed(failure) => {
            let (text, colour, tone) = failed(texts, failure, name());
            close = outcome_window(
                ui,
                texts,
                (OUTCOME, ModelExportText::FailedTitle.into(), tone),
                |ui| {
                    ui.colored_label(colour, text);
                },
            );
        }
    }
    if cancel {
        model.cancel_assembly_export();
    }
    if close {
        model.close_assembly_export();
    }
}

/// Returns the titles of the progress window and of the summary of an Entity, or of a container
/// whose Interior is exported on its own.
const fn titles(container: bool, form: AssemblyExportForm) -> (T, T) {
    match (container, form) {
        (_, AssemblyExportForm::Package) => (T::RunningTitlePackage, T::SummaryTitlePackage),
        (true, AssemblyExportForm::Glb) => (T::RunningTitleInterior, T::SummaryTitleInterior),
        (false, AssemblyExportForm::Glb) => (T::RunningTitle, T::SummaryTitle),
    }
}

pub(super) fn failed(
    texts: &Texts,
    failure: &ModelExportFailure,
    name: (&str, FluentValue<'static>),
) -> (String, Color32, Tone) {
    use ModelExportText as M;
    match failure {
        ModelExportFailure::KeyRequired => (
            texts.format(M::KeyRequired, &[name]),
            palette::WARNING,
            Tone::Normal,
        ),
        ModelExportFailure::KeyRejected => (
            texts.format(M::KeyRejected, &[name]),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::ArchiveChanged => (
            texts.format(M::ArchiveChanged, &[name]),
            palette::WARNING,
            Tone::Normal,
        ),
        ModelExportFailure::Refused(refused) => (
            crate::installation::message(texts, refused),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::Failed(reason) => (
            texts.format(M::Failed, &[name, ("reason", reason.clone().into())]),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::NoRecord(guid) => (
            texts.format(
                M::Failed,
                &[name, ("reason", no_record(texts, *guid).into())],
            ),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
        ModelExportFailure::Stopped => (
            texts.format(M::Stopped, &[name]),
            palette::DANGER_TEXT,
            Tone::Danger,
        ),
    }
}

fn running(
    ui: &mut Ui,
    texts: &Texts,
    (name, progress, package): (&str, AssemblyExportProgress, bool),
    cancelling: bool,
) -> bool {
    let name = ("name", FluentValue::from(name.to_owned()));
    let text = match progress.stage {
        AssemblyExportStage::Resolving => texts.format(T::Resolving, &[name]),
        AssemblyExportStage::Reading => texts.format(T::Reading, &[name]),
        AssemblyExportStage::Interior => texts.format(
            T::ReadingInterior,
            &[
                name,
                ("done", number_of(texts, progress.done).into()),
                ("total", number_of(texts, progress.total).into()),
            ],
        ),
        AssemblyExportStage::Writing if package => texts.format(T::WritingPackage, &[name]),
        AssemblyExportStage::Writing => texts.format(
            T::Writing,
            &[("size", format::size(progress.done, texts.language()).into())],
        ),
    };
    ui.horizontal(|ui| {
        indicator::spinner(ui, SPINNER_INLINE);
        ui.label(text);
    });
    ui.add_space(SPACE_3);
    let width = ui.available_width().max(frames::MODAL_BODY_WIDTH);
    match stage_share(progress) {
        Some(share) => indicator::progress(ui, share, cancelling, width),
        None => indicator::indeterminate(ui, cancelling, width),
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

fn number_of(texts: &Texts, value: u64) -> String {
    format::number(value, texts.language())
}

/// The texts of one summary, formatted once per export and language.
struct Laid {
    key: (u64, Language),
    notes: Vec<(String, Color32)>,
    heading: String,
    issues: Vec<String>,
    textures_heading: String,
    textures: Vec<String>,
}

impl Laid {
    fn get(ui: &Ui, texts: &Texts, (run, summary): (u64, &AssemblyExportSummary)) -> Arc<Self> {
        let key = (run, texts.language());
        let id = Id::unique("p4kx-assembly-export-summary");
        if let Some(cached) = ui.data(|data| data.get_temp::<Arc<Self>>(id))
            && cached.key == key
        {
            return cached;
        }
        let laid = Arc::new(Self::new(key, texts, summary));
        ui.data_mut(|data| data.insert_temp(id, Arc::clone(&laid)));
        laid
    }

    fn new(key: (u64, Language), texts: &Texts, summary: &AssemblyExportSummary) -> Self {
        let (textures_heading, textures) = missing_textures(texts, summary);
        Self {
            key,
            textures_heading,
            textures,
            notes: notes(texts, summary),
            heading: texts.format(
                T::ReportHeading,
                &[("number", number(texts, summary.issues().len()).into())],
            ),
            issues: summary
                .issues()
                .iter()
                .map(|issue| {
                    let path = if issue.path.is_empty() {
                        issue.name.clone()
                    } else {
                        issue.path.clone()
                    };
                    let args = [
                        ("path", path.into()),
                        ("name", issue.name.clone().into()),
                        ("reason", issue.reason.clone().into()),
                    ];
                    texts.format(
                        if issue.skipped {
                            T::ReportSkipped
                        } else {
                            T::ReportNote
                        },
                        &args,
                    )
                })
                .collect(),
        }
    }
}

/// Returns the heading and the lines of the textures the file lacks.
fn missing_textures(texts: &Texts, summary: &AssemblyExportSummary) -> (String, Vec<String>) {
    let fallbacks = &summary.written().texture_fallbacks;
    let heading = texts.format(
        ModelExportText::SummaryTextures,
        &[("number", number(texts, fallbacks.len()).into())],
    );
    let lines = fallbacks
        .iter()
        .map(|texture| {
            texts.format(
                ModelExportText::SummaryTexture,
                &[
                    ("material", printable(&texture.material).into()),
                    ("role", texture.role.name().into()),
                    ("file", printable(&texture.file).into()),
                    ("reason", printable(&texture.reason).into()),
                ],
            )
        })
        .collect();
    (heading, lines)
}

pub(super) fn number(texts: &Texts, value: usize) -> String {
    format::number(u64::try_from(value).unwrap_or(u64::MAX), texts.language())
}

fn finished(ui: &mut Ui, laid: &Laid) {
    for (note, colour) in &laid.notes {
        ui.colored_label(*colour, note);
    }
    list(
        ui,
        (&laid.textures_heading, "assembly-export-textures"),
        &laid.textures,
        None,
        true,
    );
    list(
        ui,
        (&laid.heading, "assembly-export-report"),
        &laid.issues,
        None,
        true,
    );
}
