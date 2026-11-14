// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::health_cell_order;
use super::health_text_capture::HealthTextCell;
use crate::guid_text::CatalogGuid;
use crate::{HealthText, HealthTextRef, schema_fields};

/// Directly paired body cells of one retained diagnostic occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(super) struct ExampleBodyCells {
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) field: HealthTextCell,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) source_type: HealthTextCell,
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub(super) target_guid: SourcedValue<HealthText>,
}

impl PartialOrd for ExampleBodyCells {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExampleBodyCells {
    fn cmp(&self, other: &Self) -> Ordering {
        health_cell_order::text(&self.field, &other.field)
            .then_with(|| health_cell_order::text(&self.source_type, &other.source_type))
            .then_with(|| health_cell_order::sourced(&self.target_guid, &other.target_guid))
    }
}
