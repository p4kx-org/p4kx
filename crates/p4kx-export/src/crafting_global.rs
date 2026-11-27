// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_rule_inputs::BlueprintRules;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

#[path = "crafting_global_read.rs"]
mod read;
#[path = "crafting_global_views.rs"]
mod views;

pub(crate) use read::read;

/// One physical crafting-global Record with its original default and exclusion declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CraftingGlobalConfiguration<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    default_blueprint_selection: SourcedValue<CraftingBlueprintSelection<'source>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    dismantle_blacklist_resources:
        SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    dismantle_blacklist_entity_classes:
        SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

/// An original default-blueprint selection with its concrete type and ordered reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CraftingBlueprintSelection<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    blueprint_records: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

pub(crate) struct PreparedCraftingGlobal<'source> {
    pub(crate) configuration: CraftingGlobalConfiguration<'source>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct CraftingGlobalRead<'source> {
    pub(crate) prepared: PreparedCraftingGlobal<'source>,
    pub(crate) rules: BlueprintRules,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) complete: bool,
}

pub(crate) fn order(rows: &mut [PreparedCraftingGlobal<'_>]) {
    rows.sort_by_key(|row| {
        (
            row.configuration
                .source
                .guid
                .value()
                .map(|guid| guid.text()),
            row.configuration.source.record_index.value().copied(),
        )
    });
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<PreparedCraftingGlobal<'source>>,
) -> CatalogTable<CraftingGlobalConfiguration<'source>> {
    order(&mut rows);
    CatalogTable::new(
        forge,
        &[crate::crafting_records::GLOBALS.type_name],
        rows.into_iter().map(|row| row.configuration).collect(),
    )
}
