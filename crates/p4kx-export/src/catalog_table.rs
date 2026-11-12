// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

/// Availability of a declared source family independently of successful row count.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum DatasetAvailability {
    Available,
    Absent,
    Unavailable,
}

/// One named typed dataset with its measured source-family availability.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "CatalogTableOf{T}", transform = every_field_required)]
#[must_use]
pub(crate) struct CatalogTable<T> {
    pub(crate) availability: DatasetAvailability,
    pub(crate) rows: Vec<T>,
}

impl<T> CatalogTable<T> {
    pub(crate) const fn empty(availability: DatasetAvailability) -> Self {
        Self {
            availability,
            rows: Vec::new(),
        }
    }

    pub(crate) fn new(forge: &DataForge<'_>, source_types: &[&str], rows: Vec<T>) -> Self {
        Self::matching_source_types(forge, |name| source_types.contains(&name), rows)
    }

    pub(crate) fn matching_source_types(
        forge: &DataForge<'_>,
        matches_source_type: impl Fn(&str) -> bool,
        rows: Vec<T>,
    ) -> Self {
        let mut unavailable = false;
        let mut declared = !rows.is_empty();
        for structure in forge.structs() {
            match structure.name() {
                Ok(name) if matches_source_type(name) => declared = true,
                Ok(_) => {}
                Err(_) => unavailable = true,
            }
        }
        let availability = if declared {
            DatasetAvailability::Available
        } else if unavailable {
            DatasetAvailability::Unavailable
        } else {
            DatasetAvailability::Absent
        };
        Self { availability, rows }
    }
}

#[cfg(test)]
#[path = "catalog_table_tests.rs"]
mod tests;
