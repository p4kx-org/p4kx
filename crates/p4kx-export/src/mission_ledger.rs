// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Record;

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::selection::Kind;

const INPUT_KINDS: [Kind; 5] = [
    Kind::MissionPropertySets,
    Kind::ContractShipSpawns,
    Kind::ContractNpcSpawns,
    Kind::ContractEntitySpawns,
    Kind::ContractLocationQueries,
];

/// Records the completed Generator and sibling input inventories.
pub(crate) fn inventory(ledger: &mut ExportLedger) {
    ledger.inventory(Kind::Contracts);
    for kind in INPUT_KINDS {
        ledger.inventory(kind);
    }
}

/// Captures one Generator transaction independently of its contract and derived-row fanout.
pub(crate) fn observe(
    ledger: &mut ExportLedger,
    record: &Record<'_>,
    role: ExportSourceRole,
    disposition: SourceDisposition,
    error: Option<&(dyn std::error::Error + 'static)>,
) {
    let mut source = SourceObservation::new(Kind::Contracts, record, role, disposition);
    if let Some(error) = error {
        source = source.diagnosed(crate::export_source_failure::diagnosis(error));
    }
    ledger.observe(source.clone());
    for kind in INPUT_KINDS {
        let mut input = source.clone();
        input.kind = Some(kind);
        input.role = if role == ExportSourceRole::Selected {
            ExportSourceRole::DerivedInput
        } else {
            role
        };
        ledger.observe(input);
    }
}
