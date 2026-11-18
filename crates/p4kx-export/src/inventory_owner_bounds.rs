// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourcedValue};
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
pub struct InventoryOwnerBounds<'source> {
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: BoundsFields<'source>,
}

/// The original inventory occupancy, vehicle bound or metadata field family.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BoundsFields<'source> {
    /// Occupancy fields on the original attachment definition.
    ItemOccupancy {
        /// The original attachment definition and its retained state.
        attach_def: Box<SourcedValue<InventoryItemOccupancy<'source>>>,
    },
    /// A vehicle component's original maximum dimensions.
    VehicleBounds {
        /// The original maximum bounding-box vector.
        max_bounding_box_size: Box<SourcedValue<InventoryVector3<'source>>>,
    },
    /// The original local-bounds metadata declaration.
    ObjectMetadata {
        /// Whether the source explicitly declares local bounds.
        has_local_bounds: SourcedValue<bool>,
    },
}

impl<'source> InventoryOwnerBounds<'source> {
    /// Returns the nominal owner GUID with its original header origin.
    #[must_use]
    pub fn owner_guid(&self) -> SourcedValue<Guid> {
        self.owner.guid().map_ref(|guid| guid.0)
    }

    /// Borrows the physical Record index in the queried DataForge.
    #[must_use]
    pub const fn owner_record_index(&self) -> &SourcedValue<u32> {
        self.owner.record_index()
    }

    /// Borrows the original Components array ordinal.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        self.owner.component_ordinal()
    }

    /// Borrows the original component source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the retained bound family and its original field states.
    #[must_use]
    pub const fn fields(&self) -> &BoundsFields<'source> {
        &self.fields
    }

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
