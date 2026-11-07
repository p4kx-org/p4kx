// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDisposition};

const POOLS: [Kind; 4] = [
    Kind::ContractShipSpawns,
    Kind::ContractNpcSpawns,
    Kind::ContractEntitySpawns,
    Kind::ContractLocationQueries,
];

/// Retains broker-source outcomes for the pools produced transactionally by the full mapper.
pub(crate) fn broker(ledger: &mut ExportLedger) {
    let sources: Vec<_> = ledger
        .sources
        .iter()
        .filter(|source| source.kind == Some(Kind::MissionBrokerEntries))
        .cloned()
        .collect();
    for source in sources {
        for kind in POOLS {
            let mut input = source.clone();
            input.kind = Some(kind);
            if input.role == ExportSourceRole::Selected {
                input.role = ExportSourceRole::DerivedInput;
            }
            ledger.observe(input);
        }
    }
}

/// Retains failures of full template expansion without contradicting successful base-pool inputs.
pub(crate) fn templates(ledger: &mut ExportLedger) {
    let failed: Vec<_> = ledger
        .sources
        .iter()
        .filter(|source| {
            source.kind == Some(Kind::ContractTemplates)
                && source.role == ExportSourceRole::Selected
                && source.disposition == SourceDisposition::ProjectionFailed
        })
        .cloned()
        .collect();
    for source in failed {
        for kind in POOLS {
            ledger.derived_failure(DerivedFailure {
                affected_kind: kind,
                output_kind: Kind::ContractTemplates,
                record_index: source.record_index,
                role: ExportSourceRole::DerivedInput,
                guid: source.guid,
                record_path: source.record_path.clone(),
                diagnosis: source.diagnosis.clone(),
            });
        }
    }
}
