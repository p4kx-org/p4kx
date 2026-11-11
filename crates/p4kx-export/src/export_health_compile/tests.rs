// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "tests/counters.rs"]
mod counters;
#[path = "tests/diagnostics.rs"]
mod diagnostics;
#[path = "tests/entries.rs"]
mod entries;
#[path = "tests/membership.rs"]
mod membership;

use p4kx_extraction::Guid;

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::{Kind, KindSummary};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
#[path = "tests/original.rs"]
mod original;

fn source(
    kind: Kind,
    id: u8,
    role: ExportSourceRole,
    disposition: SourceDisposition,
) -> SourceObservation {
    SourceObservation::accounting(
        Some(kind),
        u32::from(id),
        Guid::from_stored([id; 16]),
        Some(crate::HealthText::new(&format!("synthetic/{id}"))),
        role,
        disposition,
    )
}

fn observe(ledger: &mut ExportLedger, source: SourceObservation) {
    if source.disposition.read_succeeded() {
        ledger
            .field_inspections
            .push(crate::export_field_inspection::FieldInspection {
                kind: source.kind,
                record_index: source.record_index,
                guid: source.guid,
                record_path: source
                    .record_path
                    .clone()
                    .expect("readable accounting source path"),
                subject: source.subject().clone(),
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
