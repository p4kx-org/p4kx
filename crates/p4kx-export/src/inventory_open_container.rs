// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::inventory_dimensions::InventoryVector3;
use super::inventory_grid_cell::{self, InventoryGridCellSize};
use super::inventory_item_type_filter::{self, InventoryItemTypeFilter};
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const GRID_CELL: &str = "gridCellSize";
const MIN_SIZE: &str = "minPermittedItemSize";
const MAX_SIZE: &str = "maxPermittedItemSize";
const OFFSET: &str = "gridPosOffset";
const EXTERNAL: &str = "isExternalContainer";
const DESTROYED_PERCENT: &str = "maxPercentageErasedOnParentDestruction";
const DESTRUCTION_EXPONENT: &str = "randomDestructionDistributionExponent";
const DESTRUCTION_FORCE: &str = "appliedForceOnParentDestruction";
const OVERRIDES: &str = "overrideStorableItemTypes";

/// The authored open storage grid and its independent restrictions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryOpenContainer<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    grid_cell_size: SourcedValue<InventoryGridCellSize<'source>>,
    min_permitted_item_size: SourcedValue<InventoryVector3<'source>>,
    max_permitted_item_size: SourcedValue<InventoryVector3<'source>>,
    grid_pos_offset: SourcedValue<InventoryVector3<'source>>,
    is_external_container: SourcedValue<bool>,
    max_percentage_erased_on_parent_destruction: SourcedValue<i32>,
    random_destruction_distribution_exponent: SourcedValue<ResourceFloat32>,
    applied_force_on_parent_destruction: SourcedValue<ResourceFloat32>,
    override_storable_item_types: SourcedValue<Vec<SourcedValue<InventoryItemTypeFilter<'source>>>>,
}

impl<'source> InventoryOpenContainer<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let grid_cell_size = source.strong_pointer_family_to(
            GRID_CELL,
            inventory_grid_cell::BASE,
            notes,
            InventoryGridCellSize::read,
        )?;
        let min_permitted_item_size =
            source.declared_class_to(MIN_SIZE, "Vec3", notes, InventoryVector3::read)?;
        let max_permitted_item_size =
            source.declared_class_to(MAX_SIZE, "Vec3", notes, InventoryVector3::read)?;
        let grid_pos_offset =
            source.declared_class_to(OFFSET, "Vec3", notes, InventoryVector3::read)?;
        let is_external_container = source.boolean(EXTERNAL, notes)?;
        let max_percentage_erased_on_parent_destruction = source.int32(DESTROYED_PERCENT, notes)?;
        let random_destruction_distribution_exponent =
            source.single(DESTRUCTION_EXPONENT, notes)?;
        let applied_force_on_parent_destruction = source.single(DESTRUCTION_FORCE, notes)?;
        let override_storable_item_types = source.class_array_to(
            OVERRIDES,
            inventory_item_type_filter::SOURCE_TYPE,
            notes,
            InventoryItemTypeFilter::read,
        )?;
        let source = UnitSource::read(
            source,
            [
                (GRID_CELL, grid_cell_size.state()),
                (MIN_SIZE, min_permitted_item_size.state()),
                (MAX_SIZE, max_permitted_item_size.state()),
                (OFFSET, grid_pos_offset.state()),
                (EXTERNAL, is_external_container.state()),
                (
                    DESTROYED_PERCENT,
                    max_percentage_erased_on_parent_destruction.state(),
                ),
                (
                    DESTRUCTION_EXPONENT,
                    random_destruction_distribution_exponent.state(),
                ),
                (
                    DESTRUCTION_FORCE,
                    applied_force_on_parent_destruction.state(),
                ),
                (OVERRIDES, override_storable_item_types.state()),
            ],
        )?;
        Ok(Self {
            source,
            grid_cell_size,
            min_permitted_item_size,
            max_permitted_item_size,
            grid_pos_offset,
            is_external_container,
            max_percentage_erased_on_parent_destruction,
            random_destruction_distribution_exponent,
            applied_force_on_parent_destruction,
            override_storable_item_types,
        })
    }
}
