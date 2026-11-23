// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::starmap_report::ContainerCounts;

/// Completeness of the Archive survey that can affect canonical placed names.
#[derive(Debug, Default)]
pub(crate) struct PlacementNameCoverage {
    pub(super) directory_complete: bool,
    pub(super) descriptors_complete: bool,
    pub(super) routes_complete: bool,
}

impl PlacementNameCoverage {
    pub(crate) fn complete_for_lookup(
        &self,
        counts: ContainerCounts,
        ledger: &ExportLedger,
    ) -> bool {
        let kind = Kind::StarmapObjects;
        self.complete_routes(counts)
            && ledger.inventories.contains(&kind)
            && !ledger.sources.iter().any(|record| {
                record.disposition.failed()
                    && match record.role {
                        ExportSourceRole::Selected => record.kind == Some(kind),
                        ExportSourceRole::SelectionCandidate => {
                            record.kind.is_none_or(|owner| owner == kind)
                        }
                        _ => false,
                    }
            })
    }

    pub(crate) const fn complete_routes(&self, counts: ContainerCounts) -> bool {
        self.directory_complete
            && self.descriptors_complete
            && self.routes_complete
            && counts.unreadable == 0
            && counts.malformed == 0
            && counts.missing == counts.verified_missing
    }
}

pub(crate) fn descriptor_inputs_complete(ledger: &ExportLedger) -> bool {
    let kind = Kind::StarmapObjects;
    !ledger.incomplete_inventories.contains(&kind)
        && !ledger
            .entries
            .iter()
            .any(|entry| entry.kind == kind && entry.disposition.failed())
}
