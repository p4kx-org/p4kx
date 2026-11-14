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
use crate::starmap_containers::proof::{PlacementEvidence, PlacementField};

/// The selected world-position projection of one physical starmap Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct StarmapPlacement {
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
        let context = origins.record(object.record_index);
        let proof = PlacementBinding::new(&origins.uses, object.placement_proof.as_ref())?;
        Ok(Self {
            object_guid: context
                .header_value(RecordHeaderField::Guid, SourceValue::Present(object.guid))?,
            object_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(object.record_index),
            )?,
            system: proof.cell(object.system, PlacementField::System)?,
            entity_name: proof.cell(object.entity_name, PlacementField::Entity)?,
            placed_under_guid: proof.cell(object.placed_under_guid, PlacementField::Parent)?,
            position: proof.cell(object.position, PlacementField::Position)?,
        })
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

struct PlacementBinding<'catalog> {
    sources: &'catalog SourceUses,
    placed: Option<(&'catalog PlacementEvidence, SharedFieldOrigin)>,
}

impl<'catalog> PlacementBinding<'catalog> {
    fn new(
        sources: &'catalog SourceUses,
        proof: Option<&'catalog PlacementEvidence>,
    ) -> Result<Self, CatalogError> {
        let placed = proof
            .map(|proof| proof.shared(sources).map(|common| (proof, common)))
            .transpose()?;
        Ok(Self { sources, placed })
    }

    fn cell<T>(
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
