// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, TreeBuilder};

use super::Scan;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_harvest_modules::harvest_provider;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::{self, ItemComponentsRead, resolution};
use crate::catalog_v3::catalog_weapon_projectile_ammo::{
    self, LinkedAmmunitionReader, WeaponProjectileAmmoSelection,
};
use crate::catalog_v3::mineable_entity_definition;
use crate::catalog_v3::starmap_object_definition;
use crate::contract::Item;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::item_class::ClassLine;
use crate::picked::{Picked, PickedItem};
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::ship;
use crate::starmap::starmap_object;
use crate::{CatalogError, ExportAccountingError, Kind, item_stats};

#[path = "scan_loot_projection.rs"]
mod loot_projection;

#[path = "scan_crafter_projection.rs"]
mod crafter;

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
        let primary = self.project_primary(trees, admission, tree, ledger)?;
        self.with_crafter(primary, admission, tree, ledger)
            .map_err(ScanError::from)
    }

    fn project_primary(
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
            let (entity, notes) = mineable_entity_definition::read(
                tree,
                &self.projection,
                admission,
                &self.scan.signature_slot,
                self.scan.options.cancel,
            )?;
            crate::source_projection_notes::record(
                kind,
                ExportSourceRole::Selected,
                admission,
                tree,
                notes,
                ledger,
            )?;
            return Ok(Some(Picked::MineableEntity(entity)));
        }
        if kind == Kind::StarmapObjects {
            return Ok(Some(self.starmap(trees, admission, tree, ledger)?));
        }
        if kind == Kind::Ships {
            return Ok(Some(self.ship(admission, tree, ledger)?));
        }
        if kind == Kind::Commodities {
            return Ok(Some(self.commodity(trees, admission, tree, ledger)?));
        }
        if kind == Kind::Items {
            let (definition, read, (item, line)) =
                self.item_components(tree, trees, admission, ledger)?;
            let ports = crate::loadout::carries_ports(tree).then_some(crate::loadout::Wanted {
                guid: item.identity.guid,
                kind: crate::contract_loadout::EntityKind::Item,
                record_index: tree.record_index(),
            });
            return Ok(Some(Picked::Item(Box::new(PickedItem {
                entity: item,
                line,
                ports,
                components: read.rows,
                definition,
            }))));
        }
        Ok(Picked::entity(kind, tree))
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
                crate::source_projection_notes::record(
                    Kind::Items,
                    ExportSourceRole::Selected,
                    admission,
                    tree,
                    std::mem::take(&mut read.notes),
                    ledger,
                )?;
                self.item_stats_with_magazines(tree, trees, admission, originals, read, ledger)?;
                crate::item::item(tree, definition, originals, &self.projection)
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
    ) -> Result<(), CatalogError> {
        let cancel = self.scan.options.cancel;
        let health = read.legacy_health;
        let reader = LinkedAmmunitionReader {
            projection: &self.projection,
            forge: self.scan.forge,
            cancel,
        };
        let mut projectile_ledger = ExportLedger::default();
        let mut observed_magazine = false;
        let _ = item_stats::read_with_sources(
            tree,
            self.scan.forge,
            trees,
            || health,
            |target, _| self.scan.ammo.dependency(&target.record, ledger),
            |observed| {
                observed_magazine = true;
                if let Some(row) = resolution::read(
                    self.projection.original(),
                    admission.record(),
                    tree,
                    originals,
                    observed,
                    cancel,
                    Some(&self.scan.ammo.magazines),
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
            |_, _| Ok(()),
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
        self.retain_projectile_ammo(&read.rows.projectile_ammo_selections, ledger)?;
        Ok(())
    }

    fn retain_projectile_ammo(
        &self,
        selections: &[WeaponProjectileAmmoSelection],
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        for index in selections
            .iter()
            .filter_map(WeaponProjectileAmmoSelection::selected_ammo_index)
            .filter_map(|cell| cell.value().copied())
        {
            crate::catalog_query_session::checkpoint(self.scan.options.cancel)?;
            let record = self
                .scan
                .forge
                .record(index)
                .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index })?;
            self.scan
                .ammo
                .retain_projectile_dependency(&record, ledger)?;
        }
        Ok(())
    }

    fn definition_kind(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if let Some(picked) = self.loot_definition(kind, trees, admission, tree)? {
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
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            ship::ship(
                tree,
                projected.source,
                &self.scan.evidence,
                self.scan.original,
                read,
                aggregates,
            )
        })
    }
}
