// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, TreeBuilder};
use rayon::prelude::*;

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_v3::catalog_entity_configurations::EntityConfigurationRows;
use crate::export_ledger::ExportLedger;
use crate::picked::Picked;
use crate::scan_health::ScanRead;
use crate::scan_signature::SlotProblem;
use crate::{CatalogError, Kind};

use super::Scan;

// Several records per worker amortize each window's join while bounding pending results.
const RECORDS_PER_SCAN_WORKER: usize = 64;

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn complete(
        &self,
        records: &[Record<'f>],
        mut ledger: ExportLedger,
        failures: impl IntoIterator<Item = CatalogReadFailure<'f>>,
        slot_problem: Option<SlotProblem>,
        crafting: &CraftingKinds<'f>,
    ) -> Result<ScanRead<'f>, CatalogError> {
        let mut picked = Vec::new();
        let mut entity_tags = Vec::new();
        let mut entity_configurations = EntityConfigurationRows::default();
        let window_size = rayon::current_num_threads()
            .saturating_mul(RECORDS_PER_SCAN_WORKER)
            .min(records.len())
            .max(1);
        let mut checked = Vec::new();
        for window in records.chunks(window_size) {
            window
                .par_iter()
                .map_init(TreeBuilder::new, |trees, record| self.check(trees, record))
                .collect_into_vec(&mut checked);
            #[expect(
                clippy::iter_with_drain,
                reason = "reuse the indexed Rayon result buffer across scan windows"
            )]
            for result in checked.drain(..) {
                if let Some(record) = result? {
                    picked.extend(record.picked);
                    entity_tags.extend(record.entity.tags);
                    entity_configurations.extend(record.entity.configurations);
                    ledger.merge(record.ledger);
                }
            }
        }
        picked.extend(failures.into_iter().map(Picked::Failed));
        let resources = self.resource_commodities(&picked, crafting, &ledger)?;
        picked.extend(resources.picked);
        entity_tags.extend(resources.entity_tags);
        ledger.merge(resources.ledger);
        picked.extend(self.links.warning().map(Picked::Warned));
        let has_signatures = picked.iter().any(
            |pick| matches!(pick, Picked::MineableEntity(entity) if !entity.signatures.is_empty()),
        );
        if self.options.filter.is_none() && has_signatures {
            picked.extend(slot_problem.map(|problem| Picked::Warned(problem.warning())));
        }
        let inventories = Kind::ALL.into_iter().filter(|kind| {
            !kind.has_dedicated_reader() && !kind.is_derived() && self.domain.includes(*kind)
        });
        for kind in inventories {
            ledger.inventory(kind);
        }
        Ok(ScanRead {
            picked,
            entity_tags,
            entity_configurations,
            resource_group_members: Vec::new(),
            ledger,
        })
    }
}
