// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, TreeBuilder};

use super::Scan;
use crate::commodity::resource_type_guid;
use crate::export_ledger::ExportLedger;
use crate::harvest_config::multi_config;
use crate::harvest_preset::provider_preset;
use crate::mining::mineable_entity;
use crate::picked::Picked;
use crate::scan_health::ScanError;
use crate::starmap::starmap_object;
use crate::{CatalogError, Kind, item_stats, item_variant};

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn project(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if let Some(picked) = self.harvestable(kind, trees, tree) {
            return Ok(Some(picked));
        }
        if kind == Kind::MineableEntities {
            let entity = Picked::MineableEntity(mineable_entity(tree, self.signature_slot));
            return Ok(Some(entity.crafted(tree)));
        }
        if kind == Kind::StarmapObjects {
            let object = starmap_object(
                trees,
                tree,
                self.texts,
                self.forge,
                &self.types,
                self.placements,
            );
            return Ok(Some(Picked::Starmap(object)));
        }
        if kind == Kind::Ships {
            return Ok(Some(self.ship(record, tree, ledger)?.crafted(tree)));
        }
        let resource_type = match kind {
            Kind::Commodities => self.resource_type(trees, tree)?,
            _ => None,
        };
        let mut picked =
            Picked::entity(kind, tree, self.texts, resource_type.as_ref(), &self.groups);
        if let Some(Picked::Item(item, _, _)) = &mut picked {
            item.stats = item_stats::read(tree, self.forge, trees).map(Box::new);
            item.variant_key = item_variant::key(tree, item.stats.as_deref());
        }
        Ok(picked.map(|picked| picked.crafted(tree)))
    }

    fn ship(
        &self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, CatalogError> {
        let read = self
            .ships
            .read_catalog(record, tree, ledger, self.options.cancel)?;
        ledger.merge(read.ledger);
        let mut picked = Picked::of_ship(read.read, |read| {
            crate::ship::ship(tree, self.texts, &self.evidence, read)
        });
        if let Picked::Ship(_, vehicle) = &mut picked {
            vehicle.source_record_index = Some(record.index());
        }
        Ok(picked)
    }

    /// Returns the provider preset or multi config `tree` describes when `kind` is one of the
    /// kinds that follow a harvestable to its entity.
    fn harvestable(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        tree: &RecordTree<'f>,
    ) -> Option<Picked<'f>> {
        match kind {
            Kind::HarvestableProviderPresets => Some(Picked::Provider(provider_preset(
                trees,
                tree,
                self.forge,
                &self.links,
            ))),
            Kind::SubHarvestableMultiConfigs => Some(Picked::MultiConfig(multi_config(
                trees,
                tree,
                self.forge,
                &self.links,
            ))),
            _ => None,
        }
    }

    fn resource_type(
        &self,
        trees: &mut TreeBuilder<'f>,
        commodity: &RecordTree<'f>,
    ) -> Result<Option<RecordTree<'f>>, ScanError> {
        let Some(guid) = resource_type_guid(commodity) else {
            return Ok(None);
        };
        let Some(record) = self.forge.record_by_guid(guid.0) else {
            return Ok(None);
        };
        match trees.build(&record) {
            Ok(tree) => Ok(Some(tree)),
            Err(error) => Err(error.into()),
        }
    }
}
