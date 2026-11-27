// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{PhysicalSourceRef, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

#[path = "blueprint_pool_read.rs"]
mod read;
#[path = "blueprint_pool_views.rs"]
mod views;
#[path = "blueprint_pool_weights.rs"]
mod weights;

pub(crate) use read::read;

#[cfg(test)]
#[path = "blueprint_pool_source_tests.rs"]
mod tests;

/// One physical original Blueprint pool with every authored reward slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintPoolDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    blueprint_rewards: SourcedValue<Vec<SourcedValue<BlueprintPoolEntry<'source>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

/// The original Blueprint reference, weight and additional fields of one owned reward entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintPoolEntry<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    blueprint_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    weight: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

/// The original physical definition identity used by a derived pool projection.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintPoolRef {
    input_ref: PhysicalSourceRef,
    record_index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
}

/// The positive finite-f32 weight sum of one original pool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintPoolWeights {
    pool: BlueprintPoolRef,
    total_weight: SourcedValue<ResourceFloat32>,
}

pub(crate) struct PreparedBlueprintPool<'source> {
    pub(crate) definition: BlueprintPoolDefinition<'source>,
    pub(crate) weights: BlueprintPoolWeights,
    pub(crate) numbers: Vec<CapturedNumber>,
    pub(crate) physical_guid: p4kx_extraction::Guid,
    pub(crate) physical_path: &'source str,
    pub(crate) physical_index: u32,
}

pub(crate) struct BlueprintPoolRead<'source> {
    pub(crate) prepared: PreparedBlueprintPool<'source>,
    pub(crate) interpreted: crate::crafting_types::BlueprintPool<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) complete: bool,
}
