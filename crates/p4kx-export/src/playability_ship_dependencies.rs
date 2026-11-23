// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{CancelToken, Guid, Record, RecordTree};

use super::{Evidence, manifest_guid, tag_guids};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::export_ledger::ExportLedger;

/// Retains the physical manifest and tag decisions reached by successive Ship groups.
pub(crate) struct ShipDependencies<'read, 'source> {
    input: &'read OriginalCatalogInput<'read, 'source>,
    resolved: HashSet<Guid>,
    evidence: Evidence,
}

impl<'read, 'source> ShipDependencies<'read, 'source> {
    pub(crate) fn new(input: &'read OriginalCatalogInput<'read, 'source>) -> Self {
        Self {
            input,
            resolved: HashSet::new(),
            evidence: Evidence::default(),
        }
    }

    pub(crate) const fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    /// Reads each newly reached GUID's entire physical alias set.
    pub(crate) fn prepare<'tree>(
        &mut self,
        ships: impl IntoIterator<Item = &'tree RecordTree<'source>>,
        cancel: Option<&CancelToken>,
    ) -> Result<ExportLedger, CatalogError>
    where
        'source: 'tree,
    {
        let mut targets = ship_targets(ships, cancel)?;
        targets.retain(|guid| !self.resolved.contains(guid));
        let (evidence, ledger) = read_targets(self.input, &targets, cancel)?;
        self.resolved.extend(targets);
        self.evidence.crewed.extend(evidence.crewed);
        self.evidence.unmanned.extend(evidence.unmanned);
        self.evidence.unreadable.extend(evidence.unreadable);
        self.evidence.proofs.extend(evidence.proofs);
        Ok(ledger)
    }
}

impl Evidence {
    /// Reads the physical manifest and tag aliases reached by the selected Ships.
    pub(crate) fn read_for_ships<'tree, 'source: 'tree>(
        ships: impl IntoIterator<Item = &'tree RecordTree<'source>>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, ExportLedger), CatalogError> {
        let targets = ship_targets(ships, cancel)?;
        read_targets(input, &targets, cancel)
    }
}

fn ship_targets<'tree, 'source: 'tree>(
    ships: impl IntoIterator<Item = &'tree RecordTree<'source>>,
    cancel: Option<&CancelToken>,
) -> Result<HashSet<Guid>, CatalogError> {
    let mut targets = HashSet::new();
    for tree in ships {
        checkpoint(cancel)?;
        targets.extend(manifest_guid(tree));
        targets.extend(tag_guids(tree));
    }
    Ok(targets)
}

fn read_targets(
    input: &OriginalCatalogInput<'_, '_>,
    targets: &HashSet<Guid>,
    cancel: Option<&CancelToken>,
) -> Result<(Evidence, ExportLedger), CatalogError> {
    let mut records = Vec::new();
    for guid in targets {
        checkpoint(cancel)?;
        records.extend(input.forge().records_by_guid(*guid));
    }
    records.sort_by_key(Record::index);
    let (evidence, _, ledger) = Evidence::read_captured(&records, input, cancel)?;
    Ok((evidence, ledger))
}
