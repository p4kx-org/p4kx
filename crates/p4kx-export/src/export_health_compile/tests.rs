// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod counters;
mod diagnostics;
mod entries;
mod membership;

use p4kx_extraction::Guid;

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDiagnosis, SourceDisposition, SourceObservation};
use crate::{Kind, KindSummary};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source(
    kind: Kind,
    id: u8,
    role: ExportSourceRole,
    disposition: SourceDisposition,
) -> SourceObservation {
    SourceObservation {
        kind: Some(kind),
        record_index: u32::from(id),
        guid: Guid::from_stored([id; 16]),
        record_path: Some(format!("synthetic/{id}")),
        role,
        disposition,
        diagnosis: SourceDiagnosis::default(),
    }
}

fn observe(ledger: &mut ExportLedger, source: SourceObservation) {
    if source.disposition.read_succeeded() {
        ledger
            .field_inspections
            .push(crate::export_field_inspection::FieldInspection {
                kind: source.kind,
                record_index: source.record_index,
                guid: source.guid,
                record_path: source.record_path.clone().unwrap_or_default(),
                role: source.role,
                complete: true,
                diagnostics: Vec::new(),
                number_requirements: Vec::new(),
                unresolved_references: Vec::new(),
                catalog_references: Vec::new(),
            });
    }
    ledger.observe(source);
}

fn ledger() -> ExportLedger {
    let mut ledger = ExportLedger::default();
    for kind in Kind::ALL {
        ledger.inventory(kind);
    }
    ledger
}

fn rows(kind: Kind, count: usize) -> Vec<KindSummary> {
    Kind::ALL
        .into_iter()
        .map(|found| KindSummary {
            kind: found,
            count: if found == kind { count } else { 0 },
            skipped: 0,
            complete: true,
        })
        .collect()
}
