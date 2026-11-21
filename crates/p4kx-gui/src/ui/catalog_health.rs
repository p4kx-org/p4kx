// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;
use p4kx_export::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticUnit, ExportHealth, Kind,
};

use super::extraction::number;
use crate::theme::palette;
use crate::theme::tokens::SPACE_3;
use crate::widgets::frames;
use crate::{CatalogText as T, Texts};

pub(super) fn show_entry_diagnostics(ui: &mut Ui, texts: &Texts, health: &ExportHealth) {
    let mut diagnostics = health.diagnostics().iter().filter(|diagnostic| {
        diagnostic.count_unit == ExportDiagnosticUnit::EntryOperations
            && diagnostic.severity != ExportDiagnosticSeverity::Information
    });
    let Some(first) = diagnostics.next() else {
        return;
    };
    ui.add_space(SPACE_3);
    frames::warning_alert(ui, |ui| {
        for diagnostic in std::iter::once(first).chain(diagnostics) {
            let kind = diagnostic.kind.map_or("catalog", Kind::name);
            let reason = match diagnostic.code {
                ExportDiagnosticCode::MalformedSource => "malformed",
                ExportDiagnosticCode::UnreadableSource
                | ExportDiagnosticCode::SourceReadFailure => "unreadable",
                ExportDiagnosticCode::DanglingReference => "missing",
                ExportDiagnosticCode::UnsupportedValue => "unsupported",
                _ => "failed",
            };
            let label = texts.format(
                T::HealthEntryDiagnostic,
                &[
                    ("kind", kind.into()),
                    ("reason", reason.into()),
                    ("number", number(texts, diagnostic.count)),
                    (
                        "singular",
                        if diagnostic.count == 1 { "yes" } else { "no" }.into(),
                    ),
                ],
            );
            ui.colored_label(palette::GRAY_1, label);
        }
    });
}
