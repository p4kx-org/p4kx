// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourceUses, SourceValue, SourcedValue};
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
        Ok(Self {
            object_guid: context
                .header_value(RecordHeaderField::Guid, SourceValue::Present(object.guid))?,
            object_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(object.record_index),
            )?,
            system: computed(object.system, &origins.uses),
            entity_name: computed(object.entity_name, &origins.uses),
            placed_under_guid: computed(object.placed_under_guid, &origins.uses),
            position: computed(object.position, &origins.uses),
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

fn computed<T>(value: Option<T>, sources: &SourceUses) -> SourcedValue<T> {
    let state = value.map_or(SourceValue::Absent, SourceValue::Present);
    sources.value(state, &[], false)
}
