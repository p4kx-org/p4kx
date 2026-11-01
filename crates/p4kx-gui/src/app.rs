// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod model_file;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use model_file::ModelFileDialog;
use p4kx_settings::{Settings, WindowGeometry};

use crate::ui::Dialog;
use crate::ui_file_dialog::UiFileDialog;
use crate::{
    CATALOG_FILE, CatalogText, CharacterText, ComparisonText, HeadSource, Model, QueryExportFormat,
    QueryText, Text, ViewerSupport, smoke::Frames, ui, window,
};

/// The eframe application: a [`Model`] drawn by [`ui::show`], with the open dialog and the
/// settings file around it.
#[derive(Debug)]
pub struct App {
    model: Model,
    settings_file: Option<PathBuf>,
    window: Option<WindowGeometry>,
    language_chosen: bool,
    dialog: Option<(Dialog, Receiver<Option<PathBuf>>)>,
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

    fn start_dialog(&mut self, ctx: &egui::Context, dialog: Dialog) {
        if self.dialog.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let texts = self.model.texts();
        let filters = Filters {
            archive: texts.get(Text::DialogArchiveFilter),
            dataforge: texts.get(Text::DialogDataForgeFilter),
            report: texts.get(ComparisonText::ReportFilter),
            export: texts.get(match dialog {
                Dialog::QueryExport(QueryExportFormat::Json) => QueryText::FilterJson,
                _ => QueryText::FilterCsv,
            }),
            catalog: texts.get(CatalogText::FileFilter),
            catalog_compressed: texts.get(CatalogText::FileFilterCompressed),
            chf: texts.get(CharacterText::ChfFilter),
        };
        let ui_file = UiFileDialog::of(&self.model);
        let model_file = match dialog {
            Dialog::AssemblyFile | Dialog::AssemblyPackage => {
                ModelFileDialog::of_assembly(&self.model)
            }
            Dialog::CharacterFile | Dialog::CharacterPackage => {
                ModelFileDialog::of_character(&self.model)
            }
            _ => ModelFileDialog::of(&self.model),
        };
        let ctx = ctx.clone();
        let spawned = thread::Builder::new()
            .name("file-dialog".to_owned())
            .spawn(move || {
                let path = pick(
                    dialog,
                    rfd::FileDialog::new(),
                    filters,
                    (model_file, ui_file),
                );
                if sender.send(path).is_ok() {
                    ctx.request_repaint();
                }
            });
        if spawned.is_ok() {
            self.dialog = Some((dialog, receiver));
        }
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
                settings_changed = self.model.add_manual(path);
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

    fn take_dialog_result(&mut self) -> Option<(Dialog, PathBuf)> {
        let (dialog, receiver) = self.dialog.as_ref()?;
        let dialog = *dialog;
        let picked = receiver.try_recv();
        match picked {
            Ok(path) => {
                self.dialog = None;
                path.map(|path| (dialog, path))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.dialog = None;
                None
            }
        }
    }

    /// Stores the settings into the file freshly read, so that what another window stored
    /// meanwhile stays; the saved Queries get this window's edits replayed by name. A file that
    /// cannot be read is never written over.
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

/// The names of the file types the dialogs show, in the current language.
struct Filters {
    archive: String,
    dataforge: String,
    report: String,
    export: String,
    catalog: String,
    catalog_compressed: String,
    chf: String,
}

fn pick(
    dialog: Dialog,
    picker: rfd::FileDialog,
    filters: Filters,
    (model_file, ui_file): (ModelFileDialog, UiFileDialog),
) -> Option<PathBuf> {
    match dialog {
        Dialog::AddFolder
        | Dialog::ExtractionFolder
        | Dialog::ConversionInput
        | Dialog::ConversionOutput
        | Dialog::RecordExportFolder
        | Dialog::SoundsFolder => picker.pick_folder(),
        Dialog::OpenArchive | Dialog::AddArchive | Dialog::ComparisonArchive(_) => {
            picker.add_filter(filters.archive, &["p4k"]).pick_file()
        }
        Dialog::OpenDataForge => picker.add_filter(filters.dataforge, &["dcb"]).pick_file(),
        Dialog::ComparisonReport => picker
            .add_filter(filters.report, &["json"])
            .set_file_name("comparison.json")
            .save_file(),
        Dialog::CatalogFile => picker
            .add_filter(filters.catalog_compressed, &["zst"])
            .add_filter(filters.catalog, &["json"])
            .set_file_name(CATALOG_FILE)
            .save_file(),
        Dialog::ModelFile | Dialog::AssemblyFile | Dialog::CharacterFile => model_file.save(picker),
        Dialog::AssemblyPackage | Dialog::CharacterPackage => model_file.pick_folder(picker),
        Dialog::UiFile => ui_file.save(picker),
        Dialog::CharacterChf => picker.add_filter(filters.chf, &["chf"]).pick_file(),
        Dialog::QueryExport(format) => {
            let extension = format.extension();
            picker
                .add_filter(filters.export, &[extension])
                .set_file_name(format!("query.{extension}"))
                .save_file()
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if !self.model.viewer_support().is_probed() {
            self.model.set_viewer_support(ViewerSupport::probe(frame));
        }
        let settings_changed = self.apply_dialog_result(ui);
        let requests = ui::show(ui, &mut self.model);
        if let Some(dialog) = requests.dialog {
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

/// Stores the window geometry, the recent Archives, the manual entries added in the window next
/// to those already in `settings`, and the language, which stays unset to follow the system until the user chooses
/// one.
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
