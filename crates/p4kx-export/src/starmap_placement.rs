// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SharedFieldOrigin, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::starmap_object_definition::SOURCE_TYPE;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_table::CatalogTable;
use crate::contract_starmap::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::starmap::ObjectPlacement;
use crate::starmap_containers::proof::{PlacementEvidence, PlacementField};

/// The selected world-position projection of one physical starmap Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct StarmapPlacement {
    object_guid: SourcedValue<CatalogGuid>,
    object_record_index: SourcedValue<u32>,
    system: SourcedValue<String>,
    entity_name: SourcedValue<String>,
    placed_under_guid: SourcedValue<CatalogGuid>,
    position: SourcedValue<[f64; 3]>,
}

impl StarmapPlacement {
    fn from_object(
        object: StarmapObject<'_>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        Self::from_placement(
            object.record_index,
            object.guid,
            ObjectPlacement {
                system: object.system,
                entity_name: object.entity_name,
                placed_under_guid: object.placed_under_guid,
                position: object.position,
                proof: object.placement_proof,
            },
            origins,
        )
    }

    pub(crate) fn from_placement(
        record_index: u32,
        guid: CatalogGuid,
        placement: ObjectPlacement,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        let context = origins.record(record_index);
        let proof = PlacementBinding::new(&origins.uses, placement.proof.as_ref())?;
        Ok(Self {
            object_guid: context
                .header_value(RecordHeaderField::Guid, SourceValue::Present(guid))?,
            object_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(record_index),
            )?,
            system: proof.cell(placement.system, PlacementField::System)?,
            entity_name: proof.cell(placement.entity_name, PlacementField::Entity)?,
            placed_under_guid: proof.cell(placement.placed_under_guid, PlacementField::Parent)?,
            position: proof.cell(placement.position, PlacementField::Position)?,
        })
    }

    /// Original GUID of this object Record.
    #[must_use]
    pub const fn object_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.object_guid
    }

    /// Physical index of this object Record in the Build.
    #[must_use]
    pub const fn object_record_index(&self) -> &SourcedValue<u32> {
        &self.object_record_index
    }

    /// System of the selected placement.
    #[must_use]
    pub const fn system(&self) -> &SourcedValue<String> {
        &self.system
    }

    /// Authored entity name on the selected Child.
    #[must_use]
    pub const fn entity_name(&self) -> &SourcedValue<String> {
        &self.entity_name
    }

    /// Authored GUID of the nearest preceding placement ancestor.
    #[must_use]
    pub const fn placed_under_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.placed_under_guid
    }

    /// World-space position in metres at millimetre precision.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<[f64; 3]> {
        &self.position
    }
}

pub(super) fn table(
    forge: &DataForge<'_>,
    objects: Vec<StarmapObject<'_>>,
    origins: &CatalogOrigins,
) -> Result<CatalogTable<StarmapPlacement>, CatalogError> {
    let rows = objects
        .into_iter()
        .map(|object| StarmapPlacement::from_object(object, origins))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CatalogTable::new(forge, &[SOURCE_TYPE], rows))
}

pub(crate) struct PlacementBinding<'catalog> {
    sources: &'catalog SourceUses,
    placed: Option<(&'catalog PlacementEvidence, SharedFieldOrigin)>,
}

impl<'catalog> PlacementBinding<'catalog> {
    pub(crate) fn new(
        sources: &'catalog SourceUses,
        proof: Option<&'catalog PlacementEvidence>,
    ) -> Result<Self, CatalogError> {
        let placed = proof
            .map(|proof| proof.shared(sources).map(|common| (proof, common)))
            .transpose()?;
        Ok(Self { sources, placed })
    }

    pub(crate) fn cell<T>(
        &self,
        value: Option<T>,
        field: PlacementField,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let state = value.map_or(SourceValue::Absent, SourceValue::Present);
        match &self.placed {
            Some((proof, common)) => Ok(self.sources.value_with_shared_origin(
                state,
                common,
                &proof.uses(field)?,
                true,
            )?),
            None => Ok(self.sources.value(state, &[], false)),
        }
    }
}
