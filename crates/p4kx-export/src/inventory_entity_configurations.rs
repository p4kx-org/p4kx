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
use crate::catalog_rows::staged::{OrderedComponentRows, StagedCatalogTable};
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

const BOUNDS_SOURCE_TYPES: &[&str] = &[
    inventory_item_occupancy::ITEM_DEFINITION,
    room_associations::VEHICLE,
    inventory_owner_bounds::METADATA_COMPONENT,
];

/// Authored storage selection and bounds rows collected from selected entities.
#[derive(Default)]
pub(super) struct InventoryEntityRows<'source> {
    storage: Vec<InventoryOwnerStorage<'source>>,
    bounds: Vec<InventoryOwnerBounds<'source>>,
    staged_bounds: Option<StagedCatalogTable>,
}

impl<'source> InventoryEntityRows<'source> {
    pub(super) fn item_storage(&self) -> impl Iterator<Item = &InventoryOwnerStorage<'source>> {
        self.storage
            .iter()
            .filter(|row| row.item_container_params().is_some())
    }

    pub(super) fn storage_owners(&self) -> impl Iterator<Item = &ComponentOwner> {
        self.storage.iter().map(|row| &row.owner)
    }

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

    pub(super) fn stage_bounds(
        &mut self,
        staging: &mut OrderedComponentRows<'_>,
    ) -> Result<(), CatalogError> {
        for row in std::mem::take(&mut self.bounds) {
            staging.append(row.owner.key(), &row)?;
        }
        Ok(())
    }

    pub(super) fn install_staged_bounds(
        &mut self,
        staging: OrderedComponentRows<'_>,
        forge: &DataForge<'_>,
    ) -> Result<(), CatalogError> {
        if !self.bounds.is_empty() || self.staged_bounds.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.staged_bounds = Some(staging.finish(forge, BOUNDS_SOURCE_TYPES)?);
        Ok(())
    }

    pub(super) fn into_tables(
        mut self,
        forge: &DataForge<'_>,
    ) -> Result<InventoryEntityTables<'source>, CatalogError> {
        if !self.bounds.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let item_bounds = self
            .staged_bounds
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.storage.sort_by_key(|row| row.owner.key());
        Ok(InventoryEntityTables {
            inventory_configurations: CatalogTable::new(
                forge,
                &[
                    inventory_owner_storage::ITEM_COMPONENT,
                    room_associations::VEHICLE,
                ],
                self.storage,
            ),
            item_bounds,
        })
    }
}

/// Original storage owner and bounds datasets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryEntityTables<'source> {
    inventory_configurations: CatalogTable<InventoryOwnerStorage<'source>>,
    #[schemars(with = "CatalogTable<InventoryOwnerBounds<'source>>")]
    pub(super) item_bounds: StagedCatalogTable,
}
