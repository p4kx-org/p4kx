// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::health_text_capture::HealthTextCell;
use crate::guid_text::CatalogGuid;
use crate::{HealthText, HealthTextRef, schema_fields};

/// Original Entry facts paired with this report's immutable text store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(super) struct EntryCells {
    pub(super) entry_index: SourcedValue<usize>,
    pub(super) container_entry_index: SourcedValue<usize>,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) path: HealthTextCell,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) field: HealthTextCell,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    pub(super) source_type: HealthTextCell,
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub(super) target_guid: SourcedValue<HealthText>,
}
