// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_type_contract::ResourceType;
use crate::schema_fields::every_field_required;

/// Availability of the source family independently of row count and selection.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum ResourceAvailability {
    Available,
    Absent,
    Unavailable,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceTable<'source> {
    pub(crate) availability: ResourceAvailability,
    pub(crate) rows: Vec<ResourceType<'source>>,
}

impl<'source> ResourceTable<'source> {
    pub(crate) fn new(forge: &DataForge<'_>, rows: Vec<ResourceType<'source>>) -> Self {
        let mut unavailable = false;
        let mut declared = !rows.is_empty();
        for structure in forge.structs() {
            match structure.name() {
                Ok("ResourceType") => declared = true,
                Ok(_) => {}
                Err(_) => unavailable = true,
            }
        }
        let availability = if declared {
            ResourceAvailability::Available
        } else if unavailable {
            ResourceAvailability::Unavailable
        } else {
            ResourceAvailability::Absent
        };
        Self { availability, rows }
    }
}

/// Canonical typed definition tables already projected by the v3 writer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CatalogDefinitions<'source> {
    pub(crate) resource_types: ResourceTable<'source>,
}
