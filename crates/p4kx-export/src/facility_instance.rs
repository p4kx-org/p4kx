// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_item_alignment::PhysicalPort;
use super::facility_loadout_placements::InteriorFacilityPlacement;
use super::item_component_owner::ComponentOwner;
use super::loadout_position::LoadoutPosition;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::AttachmentIdentity;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;

/// One resolved ship placement and its authored configurations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FacilityInstance {
    #[serde(flatten)]
    pub(super) ship: ShipContext,
    pub(super) source: FacilityPlacementSource,
    #[schemars(length(min = 1))]
    pub(super) configurations: Vec<FacilityConfiguration>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ShipContext {
    pub(super) ship_guid: SourcedValue<CatalogGuid>,
    pub(super) ship_record_index: SourcedValue<u32>,
}

impl ShipContext {
    pub(super) fn read(
        origins: &CatalogOrigins,
        guid: CatalogGuid,
        index: u32,
    ) -> Result<Self, CatalogError> {
        let record = origins.record(index);
        Ok(Self {
            ship_guid: record.header_value(RecordHeaderField::Guid, SourceValue::Present(guid))?,
            ship_record_index: record
                .header_selector(RecordHeaderField::RecordIndex, SourceValue::Present(index))?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum FacilityPlacementSource {
    Loadout(LoadoutFacilityPlacement),
    Interior(Box<InteriorFacilityPlacement>),
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LoadoutFacilityPlacement {
    pub(super) position: SourcedValue<LoadoutPosition>,
    pub(super) original_port: Option<PhysicalPort>,
    pub(super) attached_item: Option<AttachmentIdentity>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "owner", rename_all = "camelCase")]
pub(super) enum FacilityConfiguration {
    Facility(ComponentOwner),
    Inventory(ComponentOwner),
}
