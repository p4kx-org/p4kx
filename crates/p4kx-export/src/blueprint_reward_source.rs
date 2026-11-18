// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceDataType, SourceRecordReference};

#[path = "blueprint_reward_pool.rs"]
mod pool;
#[path = "blueprint_reward_read.rs"]
mod read;
#[path = "blueprint_reward_scope.rs"]
mod scope;
#[path = "blueprint_reward_views.rs"]
mod views;

pub(crate) use read::read;

#[cfg(test)]
#[path = "blueprint_reward_source_tests.rs"]
mod tests;

/// One original direct pool-reference occurrence and the actual enclosing reward inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintRewardSource<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    source_ordinal: SourcedValue<DecimalU64>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    pool: SourcedValue<Box<SourceRecordReference<'source>>>,
    pool_storage: SourcedValue<SourceDataType>,
    contract_id: SourcedValue<CatalogGuid>,
    contract_name: SourcedValue<&'source str>,
    chance: SourcedValue<ResourceNumber>,
    #[serde(skip)]
    #[schemars(skip)]
    chance_ordinal: Option<u64>,
}

#[derive(Default)]
pub(crate) struct BlueprintRewardRead<'source> {
    pub(crate) rows: Vec<BlueprintRewardSource<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) interpreted: Vec<crate::crafting_types::RewardSource<'source>>,
}
