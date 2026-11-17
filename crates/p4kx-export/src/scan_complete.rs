// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, Record, TreeBuilder};
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_rows::staged::{IndexedCatalogRows, OrderedComponentRows};
use crate::catalog_v3::catalog_durability;
use crate::catalog_v3::catalog_item_alignment::ItemPortStaging;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinitionStaging;
use crate::collected::Collected;
use crate::commodity_backfill::CommodityPresence;
use crate::export_field_inspection::ScanInspectionFold;
use crate::export_ledger::ExportLedger;
use crate::guid_text::GuidText;
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
        let mut staging = RecordStaging::new(&scratch, self.options.cancel);
        let evidence = self.collect_records(
            records,
            &mut collected,
            &mut ledger,
            &mut inspections,
            &mut staging,
        )?;
        collected
            .entity_configurations
            .alignment
            .install_staged_ports(staging.item_ports, self.forge)?;
        collected
            .entity_configurations
            .install_staged_interactions(staging.interactions, self.forge)?;
        collected.item_health = catalog_durability::finish_health(staging.health, self.forge)?;
        collected.failures.extend(failures);
        let resources = self.resource_commodities(evidence.commodities, &collected, &ledger)?;
        for pick in resources.picked {
            collected.take(pick, &mut staging.item_definitions)?;
        }
        for tags in resources.entity_tags {
            staging.entity_tags.append(tags.order_key(), &tags)?;
        }
        inspections.merge(&mut ledger, resources.ledger);
        if let Some(warning) = self.links.warning() {
            collected.take(Picked::Warned(warning), &mut staging.item_definitions)?;
        }
        if self.options.filter.is_none()
            && evidence.has_signatures
            && let Some(problem) = slot_problem
        {
            collected.take(
                Picked::Warned(problem.warning()),
                &mut staging.item_definitions,
            )?;
        }
        let inventories = Kind::ALL.into_iter().filter(|kind| {
            !kind.has_dedicated_reader() && !kind.is_derived() && self.domain.includes(*kind)
        });
        for kind in inventories {
            ledger.inventory(kind);
        }
        let mut ammo_ledger = ExportLedger::default();
        self.ammo.dependencies(
            |pick| collected.take(pick, &mut staging.item_definitions),
            &mut ammo_ledger,
        )?;
        collected.item_definitions = staging.item_definitions.finish(self.forge)?;
        collected.entity_tags = staging
            .entity_tags
            .finish(self.forge, &[p4kx_assembly::ENTITY_TYPE])?;
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
        staging: &mut RecordStaging<'_, 'f>,
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
                        staging.stage_health(&mut pick)?;
                        collected.take(pick, &mut staging.item_definitions)?;
                    }
                    if let Some(tags) = record.entity.tags {
                        staging.entity_tags.append(tags.order_key(), &tags)?;
                    }
                    let mut configurations = record.entity.configurations;
                    configurations
                        .alignment
                        .stage_ports(&mut staging.item_ports)?;
                    configurations.stage_interactions(&mut staging.interactions)?;
                    collected.entity_configurations.extend(configurations);
                    inspections.merge(ledger, record.ledger);
                }
            }
        }
        Ok(evidence)
    }
}

struct RecordStaging<'stage, 'source> {
    item_ports: ItemPortStaging<'stage, 'source>,
    item_definitions: ItemDefinitionStaging<'stage, 'source>,
    entity_tags: IndexedCatalogRows<'stage, (GuidText, u32)>,
    interactions: OrderedComponentRows<'stage>,
    health: OrderedComponentRows<'stage>,
}

impl<'stage> RecordStaging<'stage, '_> {
    const fn new(directory: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            item_ports: ItemPortStaging::new(directory, cancel),
            item_definitions: ItemDefinitionStaging::new(directory, cancel),
            entity_tags: IndexedCatalogRows::new(directory, cancel),
            interactions: OrderedComponentRows::new(directory, cancel),
            health: OrderedComponentRows::new(directory, cancel),
        }
    }

    fn stage_health(&mut self, mut pick: &mut Picked<'_>) -> Result<(), CatalogError> {
        loop {
            match pick {
                Picked::Crafted(inner, _) => pick = inner.as_mut(),
                Picked::Item { components, .. } => {
                    return components.durability.stage_health(&mut self.health);
                }
                _ => return Ok(()),
            }
        }
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
