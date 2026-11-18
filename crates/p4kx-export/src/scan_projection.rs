// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, TreeBuilder};

use super::Scan;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_harvest_modules::harvest_provider;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::{self, ItemComponentsRead, resolution};
use crate::catalog_v3::catalog_weapon_projectile_ammo::{self, LinkedAmmunitionReader};
use crate::catalog_v3::loot_definition_catalog::{self, LootDefinitionKind};
use crate::catalog_v3::starmap_object_definition;
use crate::contract::Item;
use crate::contract_stats::ItemStats;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::harvest_config::{is_slotpreset_path, multi_config};
use crate::item_class::ClassLine;
use crate::loot_kinds::LootPick;
use crate::mining::mineable_entity;
use crate::picked::{Picked, PickedItem};
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::ship;
use crate::starmap::starmap_object;
use crate::{CatalogError, Kind, item_stats, item_variant};

pub(super) struct SelectedProjection<'scan, 'options, 'parameters, 'source> {
    pub(super) scan: &'scan Scan<'options, 'parameters, 'source>,
    pub(super) projection: CatalogProjection<'options, 'source>,
    kind: Kind,
}

impl<'scan, 'options, 'parameters, 'source>
    SelectedProjection<'scan, 'options, 'parameters, 'source>
{
    pub(super) const fn new(scan: &'scan Scan<'options, 'parameters, 'source>, kind: Kind) -> Self {
        Self {
            scan,
            projection: scan.projection.for_kind(kind),
            kind,
        }
    }
}

impl<'f> SelectedProjection<'_, '_, '_, 'f> {
    pub(super) fn project(
        &self,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        let kind = self.kind;
        if let Some(picked) = self.definition_kind(kind, trees, admission, tree, ledger)? {
            return Ok(Some(picked));
        }
        if kind == Kind::HarvestableProviderPresets {
            return self.provider(admission, tree, ledger).map(Some);
        }
        if kind == Kind::MineableEntities {
            let entity = Picked::MineableEntity(mineable_entity(tree, self.scan.signature_slot));
            return Ok(Some(entity.crafted(tree)));
        }
        if kind == Kind::StarmapObjects {
            return Ok(Some(self.starmap(trees, admission, tree, ledger)?));
        }
        if kind == Kind::Ships {
            return Ok(Some(self.ship(admission, tree, ledger)?.crafted(tree)));
        }
        if kind == Kind::Commodities {
            return Ok(Some(
                self.commodity(trees, admission, tree, ledger)?
                    .crafted(tree),
            ));
        }
        if kind == Kind::Items {
            let (definition, read, (item, line)) =
                self.item_components(tree, trees, admission, ledger)?;
            let ports = crate::loadout::carries_ports(tree).then_some(crate::loadout::Wanted {
                guid: item.identity.guid,
                kind: crate::contract_loadout::EntityKind::Item,
                record_index: tree.record_index(),
            });
            return Ok(Some(
                Picked::Item(Box::new(PickedItem {
                    entity: item,
                    line,
                    ports,
                    components: read.rows,
                    definition,
                }))
                .crafted(tree),
            ));
        }
        Ok(Picked::entity(kind, tree).map(|picked| picked.crafted(tree)))
    }

    fn provider(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, ScanError> {
        let (row, notes) = harvest_provider::read(
            tree,
            self.scan.forge,
            self.projection.original(),
            admission,
            self.scan.options.cancel,
            &self.scan.provider_dependencies,
        )?;
        crate::source_projection_notes::record(
            Kind::HarvestableProviderPresets,
            ExportSourceRole::Selected,
            admission,
            tree,
            notes,
            ledger,
        )?;
        Ok(Picked::Provider(Box::new(row)))
    }

    fn starmap(
        &self,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, ScanError> {
        let (read, notes) = starmap_object_definition::read(
            tree,
            &self.projection,
            admission,
            self.scan.options.cancel,
            |name| {
                starmap_object(
                    trees,
                    tree,
                    name,
                    self.scan.forge,
                    &self.scan.types,
                    self.scan.domain.placements,
                )
            },
        )?;
        crate::source_projection_notes::record(
            Kind::StarmapObjects,
            ExportSourceRole::Selected,
            admission,
            tree,
            notes,
            ledger,
        )?;
        Ok(Picked::Starmap(Box::new(read)))
    }

    fn item_components(
        &self,
        tree: &RecordTree<'f>,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        ledger: &mut ExportLedger,
    ) -> Result<
        (
            ItemDefinition<'f>,
            ItemComponentsRead<'f>,
            (Item<'f>, ClassLine),
        ),
        CatalogError,
    > {
        let cancel = self.scan.options.cancel;
        catalog_item_components::read(
            tree,
            &self.projection,
            admission,
            cancel,
            |definition, originals, read| {
                let stats = self
                    .item_stats_with_magazines(tree, trees, admission, originals, read, ledger)?;
                let (mut item, line) =
                    crate::item::item(tree, definition, originals, &self.projection)?;
                item.variant_key = item_variant::key(tree, stats.as_ref());
                Ok((item, line))
            },
        )
    }

    fn item_stats_with_magazines(
        &self,
        tree: &RecordTree<'f>,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        originals: &crate::catalog_read::original::OriginalFlowFields<'_, '_, 'f>,
        read: &mut ItemComponentsRead<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<ItemStats<'f>>, CatalogError> {
        let cancel = self.scan.options.cancel;
        crate::source_projection_notes::record(
            Kind::Items,
            ExportSourceRole::Selected,
            admission,
            tree,
            std::mem::take(&mut read.notes),
            ledger,
        )?;
        let health = read.legacy_health;
        let reader = LinkedAmmunitionReader {
            projection: &self.projection,
            forge: self.scan.forge,
            cancel,
        };
        let mut projectile_ledger = ExportLedger::default();
        let mut observed_magazine = false;
        let stats = item_stats::read_with_sources(
            tree,
            self.scan.forge,
            trees,
            || health,
            |record, _| self.scan.ammo.dependency(record, ledger),
            |observed| {
                observed_magazine = true;
                if let Some(row) = resolution::read(
                    self.projection.original(),
                    admission.record(),
                    tree,
                    originals,
                    observed,
                    cancel,
                    &self.scan.ammo.magazines,
                )? {
                    read.rows.magazine_resolutions.push(row);
                }
                read.rows.projectile_ammo_selections = catalog_weapon_projectile_ammo::read(
                    originals,
                    &reader,
                    &read.rows.weapon_actions,
                    &read.rows.ammo_bindings,
                    Some(observed),
                    &mut projectile_ledger,
                )?;
                Ok(())
            },
        )?;
        if !observed_magazine {
            read.rows.projectile_ammo_selections = catalog_weapon_projectile_ammo::read(
                originals,
                &reader,
                &read.rows.weapon_actions,
                &read.rows.ammo_bindings,
                None,
                &mut projectile_ledger,
            )?;
        }
        ledger.merge(projectile_ledger);
        Ok(stats)
    }

    fn definition_kind(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if let Some(loot) = LootDefinitionKind::of(kind) {
            let value = loot_definition_catalog::read(
                admission.record(),
                tree,
                &self.projection,
                self.scan.options.cancel,
            )?;
            let definition = Box::new(value);
            let picked = if kind == Kind::SubHarvestableMultiConfigs {
                let path = tree.record_path();
                let projection = (is_slotpreset_path(path)
                    && !crate::selection::is_leftover(path, tree.name()))
                .then(|| multi_config(trees, tree, self.scan.forge, &self.scan.links));
                Picked::MultiConfig {
                    definition,
                    projection,
                }
            } else {
                Picked::Loot(LootPick::Definition(loot, definition))
            };
            return Ok(Some(picked));
        }
        let Some((picked, notes)) = self.read_definition(kind, admission, tree)? else {
            return Ok(None);
        };
        crate::source_projection_notes::record(
            kind,
            ExportSourceRole::Selected,
            admission,
            tree,
            notes,
            ledger,
        )?;
        Ok(Some(picked))
    }

    fn ship(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, CatalogError> {
        let projected = ship::source_projection::read(
            &self.scan.ships,
            &self.projection,
            self.scan.original,
            admission,
            tree,
            ledger,
            self.scan.options.cancel,
        )?;
        Picked::of_ship(projected.read, |read| {
            let aggregates = projected
                .aggregates
                .ok_or(crate::ExportAccountingError::UnobservedFailure)?;
            ship::ship(
                tree,
                projected.source,
                &self.scan.evidence,
                read,
                aggregates,
            )
        })
    }
}
