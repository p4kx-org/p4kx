// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_item_alignment::PhysicalPort;
use super::item_component_owner::ComponentOwner;
use super::loadout_position::LoadoutPosition;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::AttachmentIdentity;
use crate::schema_fields::every_field_required;

/// One authored facility component in its resolved ship context.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FacilityInstance {
    #[serde(flatten)]
    pub(super) ship: ShipContext,
    pub(super) source: FacilityPlacementSource,
    pub(super) configuration: FacilityConfiguration,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ShipContext {
    pub(super) ship_guid: SourcedValue<CatalogGuid>,
    pub(super) ship_record_index: SourcedValue<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum FacilityPlacementSource {
    Loadout(LoadoutFacilityPlacement),
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LoadoutFacilityPlacement {
    pub(super) position: SourcedValue<LoadoutPosition>,
    pub(super) original_port: Option<PhysicalPort>,
    pub(super) attached_item: Option<AttachmentIdentity>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "owner", rename_all = "camelCase")]
pub(super) enum FacilityConfiguration {
    Facility(ComponentOwner),
    Inventory(ComponentOwner),
}
