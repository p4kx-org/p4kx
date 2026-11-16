// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, TreeBuilder};
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_v3::catalog_item_alignment::ItemPortStaging;
use crate::collected::Collected;
use crate::commodity_backfill::CommodityPresence;
use crate::export_field_inspection::ScanInspectionFold;
use crate::export_ledger::ExportLedger;
use crate::picked::Picked;
use crate::record_read;

use crate::scan_signature::SlotProblem;
use crate::{CatalogError, Kind};

use super::Scan;

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn complete(
        &self,
        records: &[Record<'f>],
        mut ledger: ExportLedger,
        failures: impl IntoIterator<Item = CatalogReadFailure<'f>>,
        slot_problem: Option<SlotProblem>,
        crafting: CraftingKinds<'f>,
    ) -> Result<Collected<'f>, CatalogError> {
        let mut inspections = ScanInspectionFold::start(&mut ledger);
        let mut collected = Collected::from_crafting(crafting);
        let scratch = std::env::temp_dir();
        let mut item_ports = ItemPortStaging::new(&scratch, self.options.cancel);
        let evidence = self.collect_records(
            records,
            &mut collected,
            &mut ledger,
            &mut inspections,
            &mut item_ports,
        )?;
        collected
            .entity_configurations
            .alignment
            .install_staged_ports(item_ports, self.forge)?;
        for failure in failures {
            collected.take(Picked::Failed(failure));
        }
        let resources = self.resource_commodities(evidence.commodities, &collected, &ledger)?;
        for pick in resources.picked {
            collected.take(pick);
        }
        collected.tags.entity_tags.extend(resources.entity_tags);
        inspections.merge(&mut ledger, resources.ledger);
        if let Some(warning) = self.links.warning() {
            collected.take(Picked::Warned(warning));
        }
        if self.options.filter.is_none()
            && evidence.has_signatures
            && let Some(problem) = slot_problem
        {
            collected.take(Picked::Warned(problem.warning()));
        }
        let inventories = Kind::ALL.into_iter().filter(|kind| {
            !kind.has_dedicated_reader() && !kind.is_derived() && self.domain.includes(*kind)
        });
        for kind in inventories {
            ledger.inventory(kind);
        }
        let mut ammo_ledger = ExportLedger::default();
        self.ammo
            .dependencies(|pick| collected.take(pick), &mut ammo_ledger)?;
        inspections.merge(&mut ledger, ammo_ledger);
        collected.ledger.merge(ledger);
        Ok(collected)
    }

    fn collect_records(
        &self,
        records: &[Record<'f>],
        collected: &mut Collected<'f>,
        ledger: &mut ExportLedger,
        inspections: &mut ScanInspectionFold,
        item_ports: &mut ItemPortStaging<'_, 'f>,
    ) -> Result<RecordEvidence, CatalogError> {
        let mut evidence = RecordEvidence {
            commodities: CommodityPresence::new(),
            has_signatures: false,
        };
        let window_size = record_read::window_size(records.len());
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
                    if let Some(mut pick) = record.picked {
                        evidence.observe(&mut pick);
                        collected.take(pick);
                    }
                    collected.tags.entity_tags.extend(record.entity.tags);
                    let mut configurations = record.entity.configurations;
                    configurations.alignment.stage_ports(item_ports)?;
                    collected.entity_configurations.extend(configurations);
                    inspections.merge(ledger, record.ledger);
                }
            }
        }
        Ok(evidence)
    }
}

struct RecordEvidence {
    commodities: CommodityPresence,
    has_signatures: bool,
}

impl RecordEvidence {
    fn observe(&mut self, pick: &mut Picked<'_>) {
        self.commodities.observe(pick);
        self.has_signatures |=
            matches!(pick, Picked::MineableEntity(entity) if !entity.signatures.is_empty());
    }
}
