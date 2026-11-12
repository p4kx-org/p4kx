// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::inventory_item_occupancy;
use super::inventory_owner_bounds::{self, InventoryOwnerBounds};
use super::inventory_owner_storage::{self, InventoryOwnerStorage};
use super::item_component_owner::ComponentOwner;
use super::room_associations;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Authored storage selection and bounds rows collected from selected entities.
#[derive(Default)]
pub(super) struct InventoryEntityRows<'source> {
    storage: Vec<InventoryOwnerStorage<'source>>,
    bounds: Vec<InventoryOwnerBounds<'source>>,
}

impl<'source> InventoryEntityRows<'source> {
    pub(super) fn extend(&mut self, other: Self) {
        self.storage.extend(other.storage);
        self.bounds.extend(other.bounds);
    }

    pub(super) fn storage(
        &mut self,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        self.storage
            .push(InventoryOwnerStorage::item(owner, source, notes)?);
        Ok(())
    }

    pub(super) fn occupancy(
        &mut self,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        self.bounds
            .push(InventoryOwnerBounds::item(owner, source, notes)?);
        Ok(())
    }

    pub(super) fn metadata(
        &mut self,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        self.bounds
            .push(InventoryOwnerBounds::metadata(owner, source, notes)?);
        Ok(())
    }

    pub(super) fn vehicle(
        &mut self,
        owner: &ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        if source.has_member(inventory_owner_storage::VEHICLE_FIELD)? {
            self.storage.push(InventoryOwnerStorage::vehicle(
                owner.clone(),
                source,
                notes,
            )?);
        }
        if source.has_member(inventory_owner_bounds::VEHICLE_FIELD)? {
            self.bounds
                .push(InventoryOwnerBounds::vehicle(owner.clone(), source, notes)?);
        }
        Ok(())
    }

    pub(super) fn into_tables(mut self, forge: &DataForge<'_>) -> InventoryEntityTables<'source> {
        self.storage.sort_by_key(|row| row.owner.key());
        self.bounds.sort_by_key(|row| row.owner.key());
        InventoryEntityTables {
            inventory_configurations: CatalogTable::new(
                forge,
                &[
                    inventory_owner_storage::ITEM_COMPONENT,
                    room_associations::VEHICLE,
                ],
                self.storage,
            ),
            item_bounds: CatalogTable::new(
                forge,
                &[
                    inventory_item_occupancy::ITEM_DEFINITION,
                    room_associations::VEHICLE,
                    inventory_owner_bounds::METADATA_COMPONENT,
                ],
                self.bounds,
            ),
        }
    }
}

/// Original storage owner and bounds datasets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryEntityTables<'source> {
    inventory_configurations: CatalogTable<InventoryOwnerStorage<'source>>,
    item_bounds: CatalogTable<InventoryOwnerBounds<'source>>,
}
