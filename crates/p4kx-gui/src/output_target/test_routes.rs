// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::QueryExportFormat;
use crate::dialog::Dialog;

pub(crate) const OUTPUT_DIALOGS: [Dialog; 11] = [
    Dialog::ModelFile,
    Dialog::AssemblyFile,
    Dialog::AssemblyPackage,
    Dialog::CharacterFile,
    Dialog::CharacterPackage,
    Dialog::UiFile,
    Dialog::CatalogFile,
    Dialog::DiagnosticCatalogFile,
    Dialog::ComparisonReport,
    Dialog::QueryExport(QueryExportFormat::Csv),
    Dialog::QueryExport(QueryExportFormat::Json),
];
