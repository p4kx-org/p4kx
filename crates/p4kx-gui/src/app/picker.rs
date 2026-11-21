// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use super::model_file::ModelFileDialog;
use crate::dialog::Dialog;
use crate::output_target::ParentTicket;
use crate::ui_file_dialog::UiFileDialog;
use crate::{
    CATALOG_FILE, CatalogText, CharacterText, ComparisonText, DIAGNOSTIC_CATALOG_FILE, Model,
    QueryExportFormat, QueryText, Text,
};

#[derive(Debug)]
pub(super) struct NativeDialog {
    pub(super) dialog: Dialog,
    pub(super) receiver: Receiver<Option<PathBuf>>,
    pub(super) parent: Option<ParentTicket>,
}

#[derive(Debug)]
pub(super) struct NativeStartError {
    pub(super) parent: Option<ParentTicket>,
    pub(super) source: std::io::Error,
}

impl NativeDialog {
    pub(super) fn start(
        ctx: &egui::Context,
        model: &Model,
        dialog: Dialog,
        parent: Option<ParentTicket>,
    ) -> Result<Self, NativeStartError> {
        let (sender, receiver) = mpsc::channel();
        let filters = Filters::of(model, dialog);
        let ui_file = UiFileDialog::of(model);
        let model_file = match dialog {
            Dialog::AssemblyFile | Dialog::AssemblyPackage => ModelFileDialog::of_assembly(model),
            Dialog::CharacterFile | Dialog::CharacterPackage => {
                ModelFileDialog::of_character(model)
            }
            _ => ModelFileDialog::of(model),
        };
        let parent_picker = parent.is_some();
        let ctx = ctx.clone();
        let spawned = thread::Builder::new()
            .name("file-dialog".to_owned())
            .spawn(move || {
                let picker = rfd::FileDialog::new();
                let path = if parent_picker {
                    picker.pick_folder()
                } else {
                    pick(dialog, picker, filters, (model_file, ui_file))
                };
                if sender.send(path).is_ok() {
                    ctx.request_repaint();
                }
            });
        match spawned {
            Ok(_) => Ok(Self {
                dialog,
                receiver,
                parent,
            }),
            Err(source) => Err(NativeStartError { parent, source }),
        }
    }
}

struct Filters {
    archive: String,
    dataforge: String,
    report: String,
    export: String,
    catalog: String,
    catalog_compressed: String,
    chf: String,
}

impl Filters {
    fn of(model: &Model, dialog: Dialog) -> Self {
        let texts = model.texts();
        Self {
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
        }
    }
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
        Dialog::CatalogFile | Dialog::DiagnosticCatalogFile => picker
            .add_filter(filters.catalog_compressed, &["zst"])
            .add_filter(filters.catalog, &["json"])
            .set_file_name(if dialog == Dialog::DiagnosticCatalogFile {
                DIAGNOSTIC_CATALOG_FILE
            } else {
                CATALOG_FILE
            })
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
