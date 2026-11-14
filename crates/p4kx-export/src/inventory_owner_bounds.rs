// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::inventory_dimensions::InventoryVector3;
use super::inventory_item_occupancy::{self, InventoryItemOccupancy};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const VEHICLE_FIELD: &str = "maxBoundingBoxSize";
pub(super) const METADATA_COMPONENT: &str = "SEntityComponentObjectMetadataParams";

/// A storage occupancy or bound field at its original component slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryOwnerBounds<'source> {
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: BoundsFields<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum BoundsFields<'source> {
    ItemOccupancy {
        attach_def: Box<SourcedValue<InventoryItemOccupancy<'source>>>,
    },
    VehicleBounds {
        max_bounding_box_size: Box<SourcedValue<InventoryVector3<'source>>>,
    },
    ObjectMetadata {
        has_local_bounds: SourcedValue<bool>,
    },
}

impl<'source> InventoryOwnerBounds<'source> {
    pub(super) fn item(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fields: BoundsFields::ItemOccupancy {
                attach_def: Box::new(source.owned(
                    inventory_item_occupancy::ATTACHMENT,
                    &[inventory_item_occupancy::ITEM_DEFINITION],
                    notes,
                    InventoryItemOccupancy::read,
                )?),
            },
        })
    }

    pub(super) fn vehicle(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fields: BoundsFields::VehicleBounds {
                max_bounding_box_size: Box::new(InventoryVector3::class_field(
                    source,
                    VEHICLE_FIELD,
                    notes,
                )?),
            },
        })
    }

    pub(super) fn metadata(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fields: BoundsFields::ObjectMetadata {
                has_local_bounds: source.boolean("hasLocalBounds", notes)?,
            },
        })
    }
}
