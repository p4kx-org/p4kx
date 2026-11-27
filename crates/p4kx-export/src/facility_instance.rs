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
pub struct FacilityInstance {
    #[serde(flatten)]
    pub(super) ship: ShipContext,
    pub(super) source: FacilityPlacementSource,
    #[schemars(length(min = 1))]
    pub(super) configurations: Vec<FacilityConfiguration>,
}

impl FacilityInstance {
    /// Borrows the owning physical Ship Record index.
    #[must_use]
    pub const fn ship_record_index(&self) -> &SourcedValue<u32> {
        &self.ship.ship_record_index
    }

    /// Borrows the owning Ship GUID.
    #[must_use]
    pub const fn ship_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.ship.ship_guid
    }

    /// Borrows the resolved position of a Loadout placement.
    #[must_use]
    pub const fn loadout_position(&self) -> Option<&SourcedValue<LoadoutPosition>> {
        match &self.source {
            FacilityPlacementSource::Loadout(loadout) => Some(&loadout.position),
            FacilityPlacementSource::Interior(_) => None,
        }
    }

    /// Borrows the physical port of an attached Loadout placement.
    #[must_use]
    pub const fn original_port(&self) -> Option<&PhysicalPort> {
        match &self.source {
            FacilityPlacementSource::Loadout(loadout) => loadout.original_port.as_ref(),
            FacilityPlacementSource::Interior(_) => None,
        }
    }

    /// Borrows the resolved attachment identity of an attached Loadout placement.
    #[must_use]
    pub const fn attached_item(&self) -> Option<&AttachmentIdentity> {
        match &self.source {
            FacilityPlacementSource::Loadout(loadout) => loadout.attached_item.as_ref(),
            FacilityPlacementSource::Interior(_) => None,
        }
    }

    /// Borrows the resolved SOC Entity placement of an Interior instance.
    #[must_use]
    pub const fn interior_placement(&self) -> Option<&InteriorFacilityPlacement> {
        match &self.source {
            FacilityPlacementSource::Interior(interior) => Some(interior),
            FacilityPlacementSource::Loadout(_) => None,
        }
    }

    /// Iterates linked configuration owners in original Components array order.
    pub fn configurations(
        &self,
    ) -> impl ExactSizeIterator<Item = (FacilityConfigurationKind, &ComponentOwner)> + '_ {
        self.configurations.iter().map(|link| match link {
            FacilityConfiguration::Facility(owner) => (FacilityConfigurationKind::Facility, owner),
            FacilityConfiguration::Inventory(owner) => {
                (FacilityConfigurationKind::Inventory, owner)
            }
        })
    }
}

/// The kind of configuration admitted at a facility placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FacilityConfigurationKind {
    /// An authored Facility configuration.
    Facility,
    /// An authored Inventory storage configuration.
    Inventory,
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

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "owner", rename_all = "camelCase")]
pub(crate) enum FacilityConfiguration {
    Facility(ComponentOwner),
    Inventory(ComponentOwner),
}
