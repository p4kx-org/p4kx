// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod model_file;
#[cfg(test)]
mod output_target_character_tests;
mod output_target_lifecycle;
mod output_target_route;
#[cfg(test)]
mod output_target_tests;
mod picker;
#[cfg(test)]
mod picker_failure_tests;

use std::path::{Path, PathBuf};

use p4kx_export::CatalogPublicationMode;
use p4kx_settings::{Settings, WindowGeometry};

use crate::smoke::Frames;
use crate::ui::Dialog;
use crate::{CatalogExportOptions, HeadSource, Model, ViewerSupport, ui, window};

/// The eframe application: a [`Model`] drawn by [`ui::show`], with the open dialog and the
/// settings file around it.
#[derive(Debug)]
pub struct App {
    model: Model,
    settings_file: Option<PathBuf>,
    window: Option<WindowGeometry>,
    language_chosen: bool,
    dialog: Option<picker::NativeDialog>,
    dialog_failure: Option<crate::output_target::PickerFailure>,
    output_target: crate::output_target::TargetSelection,
    runtime: p4kx_extraction::RuntimeEnvironment,
    smoke: Option<Frames>,
}

impl App {
    /// Wraps `model`, remembering language, window geometry, recent Archives and manual entries
    /// in `settings_file`.
    #[must_use]
    pub const fn new(
        model: Model,
        settings_file: Option<PathBuf>,
        window: Option<WindowGeometry>,
    ) -> Self {
        Self {
            model,
            settings_file,
            window,
            language_chosen: false,
            dialog: None,
            dialog_failure: None,
            output_target: crate::output_target::TargetSelection::new(),
            runtime: p4kx_extraction::RuntimeEnvironment::Native,
            smoke: None,
        }
    }

    /// Makes the window close itself once it has drawn `frames`, for a smoke run.
    #[must_use]
    pub const fn with_smoke(mut self, frames: Frames) -> Self {
        self.smoke = Some(frames);
        self
    }

    /// Counts a drawn frame of a smoke run, keeps the frames coming and closes the window after
    /// the last one.
    fn advance_smoke(&mut self, ctx: &egui::Context) {
        let Some(frames) = &mut self.smoke else {
            return;
        };
        if frames.drawn() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ctx.request_repaint();
        }
    }

    /// Returns the model.
    #[must_use]
    pub const fn model(&self) -> &Model {
        &self.model
    }

    fn choose_export_file(&mut self, dialog: Dialog, path: PathBuf) {
        match dialog {
            Dialog::AssemblyFile => {
                if let Some(setup) = self.model.assembly_export_setup_mut() {
                    setup.set_file(path);
                }
            }
            Dialog::AssemblyPackage => {
                if let Some(setup) = self.model.assembly_export_setup_mut() {
                    setup.set_package_folder(path);
                }
            }
            _ => {
                if let Some(setup) = self.model.model_export_setup_mut() {
                    setup.set_file(path);
                }
            }
        }
    }

    #[expect(clippy::too_many_lines, reason = "one arm per dialog")]
    fn apply_dialog_result(&mut self, ui: &egui::Ui) -> bool {
        let mut settings_changed = false;
        match self.take_dialog_result() {
            Some((Dialog::OpenArchive | Dialog::OpenDataForge, path)) => {
                ui::open(ui, &mut self.model, path);
            }
            Some((Dialog::AddFolder | Dialog::AddArchive, path)) => {
                let ctx = ui.ctx().clone();
                settings_changed = self
                    .model
                    .add_manual_with_wake(path, move || ctx.request_repaint());
            }
            Some((Dialog::ExtractionFolder, path)) => {
                if let Some(setup) = self.model.extraction_setup_mut() {
                    setup.set_folder(path);
                }
            }
            Some((Dialog::ConversionInput, path)) => {
                self.model.begin_folder_conversion(path);
                self.start_dialog(ui.ctx(), Dialog::ConversionOutput);
            }
            Some((Dialog::ConversionOutput, path)) => {
                let ctx = ui.ctx().clone();
                self.model
                    .start_folder_conversion(path, move || ctx.request_repaint());
            }
            Some((Dialog::RecordExportFolder, path)) => {
                if let Some(setup) = self.model.record_export_setup_mut() {
                    setup.set_folder(path);
                }
            }
            Some((Dialog::SoundsFolder, path)) => {
                if let Some(setup) = self.model.sounds_mut().export_setup_mut() {
                    setup.set_folder(path);
                }
            }
            Some((
                dialog @ (Dialog::ModelFile | Dialog::AssemblyFile | Dialog::AssemblyPackage),
                path,
            )) => {
                self.choose_export_file(dialog, path);
            }
            Some((Dialog::CharacterChf, path)) => {
                self.model
                    .characters_mut()
                    .choose_head(HeadSource::File(path));
            }
            Some((Dialog::CharacterFile, path)) => {
                if let Some(setup) = self.model.characters_mut().export_setup_mut() {
                    setup.set_file(path);
                }
            }
            Some((Dialog::CharacterPackage, path)) => {
                if let Some(setup) = self.model.characters_mut().export_setup_mut() {
                    setup.set_package_folder(path);
                }
            }
            Some((Dialog::UiFile, path)) => {
                let ctx = ui.ctx().clone();
                self.model
                    .start_ui_export(path, move || ctx.request_repaint());
            }
            Some((Dialog::CatalogFile, path)) => {
                let ctx = ui.ctx().clone();
                self.model
                    .start_catalog(path, move || ctx.request_repaint());
            }
            Some((Dialog::DiagnosticCatalogFile, path)) => {
                let ctx = ui.ctx().clone();
                self.model.start_catalog_with_options(
                    path,
                    CatalogExportOptions {
                        publication_mode: CatalogPublicationMode::DiagnosticPartial,
                        ..CatalogExportOptions::default()
                    },
                    move || ctx.request_repaint(),
                );
            }
            Some((Dialog::ComparisonArchive(side), path)) => {
                self.model.comparison_mut().set_archive(side, path);
            }
            Some((Dialog::QueryExport(format), path)) => {
                let ctx = ui.ctx().clone();
                self.model
                    .query_mut()
                    .export(path, format, move || ctx.request_repaint());
            }
            Some((Dialog::ComparisonReport, path)) => {
                let ctx = ui.ctx().clone();
                if let Some(view) = self.model.comparison_mut().view_mut() {
                    view.save_report(path, move || ctx.request_repaint());
                }
            }
            None => {}
        }
        settings_changed
    }

    /// Merges this window's settings and named Query edits into the freshly loaded file.
    /// An unreadable settings file is never overwritten.
    fn save_settings(&mut self) {
        let Some(file) = &self.settings_file else {
            return;
        };
        let Ok(mut settings) = Settings::load(file) else {
            return;
        };
        remember(
            &mut settings,
            &self.model,
            self.language_chosen,
            self.window,
        );
        let edits = self.model.query().saved_edits();
        for edit in edits {
            settings.queries.apply(edit);
        }
        let edited = !edits.is_empty();
        if settings.save(file).is_ok() && edited {
            self.model.query_mut().edits_stored(settings.queries);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if !self.model.viewer_support().is_probed() {
            self.model.set_viewer_support(ViewerSupport::probe(frame));
        }
        let settings_changed = self.apply_dialog_result(ui);
        let requests = ui::show_with_output_target(
            ui,
            &mut self.model,
            self.output_target.pending().is_some() || self.dialog_failure.is_some(),
        );
        self.retain_output_target();
        let parent = if self.dialog_failure.is_none() {
            ui::output_target(ui, self.model.texts(), &mut self.output_target)
        } else {
            None
        };
        ui::picker_failure(ui, self.model.texts(), &mut self.dialog_failure);
        if let Some(dialog) = requests.dialog.or(parent) {
            self.start_dialog(ui.ctx(), dialog);
        }
        if requests.language_changed {
            self.language_chosen = true;
        }
        if settings_changed || requests.settings_changed || requests.language_changed {
            self.save_settings();
        }
        self.window = ui.input(|input| window::geometry(self.window, input.viewport()));
        self.advance_smoke(ui.ctx());
    }

    fn on_exit(&mut self) {
        self.model.stop_extraction();
        self.model.stop_record_export();
        self.model.sounds_mut().stop_export();
        self.model.stop_model_export();
        self.model.stop_assembly_export();
        self.model.characters_mut().stop_export();
        self.model.stop_catalog();
        self.model.comparison_mut().cancel();
        self.model.query_mut().cancel();
        self.save_settings();
        crate::exit_guard::arm(
            self.settings_file
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        );
    }
}

/// Stores window/export preferences and recent Archives, merging manual entries into settings.
/// The language follows the system until explicitly chosen.
fn remember(settings: &mut Settings, model: &Model, chosen: bool, window: Option<WindowGeometry>) {
    if chosen || settings.language.is_some() {
        settings.language = Some(model.texts().language().tag().to_owned());
    }
    settings.record_language = model
        .record_text_choice()
        .map(|choice| choice.code().to_owned());
    settings.window = window;
    settings.recent = model.recent().clone();
    settings.extraction_folder = model.extraction_folder().map(Path::to_path_buf);
    settings.extraction_conversion = model.extraction_conversion();
    settings.extraction_textures = model.extraction_textures();
    settings.extraction_audio = model.extraction_audio();
    settings.update_check = model.update_check();
    settings.model_export = model.model_export_settings().clone();
    settings.assembly_export = model.assembly_export_settings().clone();
    settings.ui = model.ui_render_settings();
    settings.channels.merge(model.channels().added());
}

#[cfg(test)]
mod tests;
