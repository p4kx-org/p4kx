// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dialog::Dialog;
use crate::output_target::{OutputScope, Witness};
use crate::{AssemblyExportForm, CATALOG_FILE, DIAGNOSTIC_CATALOG_FILE, Model};

pub(super) fn scope(model: &Model, dialog: Dialog) -> Option<OutputScope> {
    if dialog == Dialog::ComparisonReport {
        return Some(OutputScope {
            source: None,
            owner: model.comparison().view()?.output_owner(),
            subject: None,
            picture: None,
        });
    }
    if model.opening_path().is_some() || model.archive_changed() {
        return None;
    }
    let source = Witness::of(model.browser()?.shared_index());
    let mut subject = None;
    let owner = match dialog {
        Dialog::ModelFile => model.model_export_setup()?.output_owner(),
        Dialog::AssemblyFile | Dialog::AssemblyPackage => {
            let setup = model.assembly_export_setup()?;
            if (setup.form() == AssemblyExportForm::Package) != dialog.is_package_output() {
                return None;
            }
            setup.output_owner()
        }
        Dialog::CharacterFile | Dialog::CharacterPackage => {
            let setup = model.characters().export_setup()?;
            if (setup.form() == AssemblyExportForm::Package) != dialog.is_package_output() {
                return None;
            }
            subject = Some(model.characters().export_subject()?);
            setup.output_owner()
        }
        Dialog::QueryExport(_) if !model.query().is_exporting() => {
            let result = model.query().result()?;
            Witness::of(result.forge()).at(result.generation(), 0)
        }
        Dialog::UiFile => source.clone(),
        Dialog::CatalogFile | Dialog::DiagnosticCatalogFile if model.catalog_available() => {
            source.clone()
        }
        _ => return None,
    };
    let picture = if dialog == Dialog::UiFile {
        Some(model.ui().output_picture(model.record_text_language())?)
    } else {
        None
    };
    Some(OutputScope {
        source: Some(source),
        owner,
        subject,
        picture,
    })
}

pub(super) fn name(model: &Model, dialog: Dialog) -> Option<String> {
    Some(match dialog {
        Dialog::ModelFile => model.model_export_setup()?.proposed_name(),
        Dialog::AssemblyFile => model.assembly_export_setup()?.proposed_name(),
        Dialog::AssemblyPackage => model.assembly_export_setup()?.name().to_owned(),
        Dialog::CharacterFile => model.characters().export_setup()?.proposed_name(),
        Dialog::CharacterPackage => model.characters().export_setup()?.name().to_owned(),
        Dialog::UiFile => model.ui().proposed_file_name(),
        Dialog::CatalogFile => CATALOG_FILE.to_owned(),
        Dialog::DiagnosticCatalogFile => DIAGNOSTIC_CATALOG_FILE.to_owned(),
        Dialog::QueryExport(format) => format!("query.{}", format.extension()),
        Dialog::ComparisonReport => "comparison.json".to_owned(),
        _ => return None,
    })
}
