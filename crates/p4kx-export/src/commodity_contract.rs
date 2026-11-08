// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::commodity_authoring_contract::CommodityAuthoring;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// The physical Commodity input and its materialization decision.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommodityDefinition<'source> {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) class_name: SourcedValue<&'source str>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) selection: SourcedValue<CommoditySelection>,
    pub(crate) selection_diagnostics: Vec<CommoditySelectionDiagnostic>,
    pub(crate) entity: SourcedValue<CommodityAuthoring<'source>>,
}

/// Why this physical Record supplies a Commodity row.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum CommoditySelection {
    Entity,
    ResourceFallback,
}

/// An origin-capture observation independent of payload and publication health.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommoditySelectionDiagnostic {
    pub(crate) code: &'static str,
    pub(crate) record_index: SourcedValue<u32>,
}
/// The established Commodity choices independently of their authored input facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommodityPresentation {
    pub(crate) commodity_guid: SourcedValue<CatalogGuid>,
    pub(crate) name: SourcedValue<Option<PublicNameRef>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<Option<GameTextRef>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_type: SourcedValue<Option<GameTextRef>>,
    pub(crate) type_guid: SourcedValue<Option<CatalogGuid>>,
    pub(crate) subtype_guid: SourcedValue<Option<CatalogGuid>>,
    pub(crate) density_grams_per_cubic_centimetre: SourcedValue<Option<f32>>,
    pub(crate) refined_subtype_guid: SourcedValue<Option<CatalogGuid>>,
}

/// Prepared source facts and choices with internal selection/accounting keys.
#[derive(Debug)]
pub(crate) struct Commodity<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) subtype_guid: Option<CatalogGuid>,
    pub(crate) definition: CommodityDefinition<'source>,
    pub(crate) presentation: CommodityPresentation,
    pub(crate) relationship_inputs: Vec<SourceUse>,
    pub(crate) crafter_inputs: Vec<SourceUse>,
    pub(crate) crafter_complete: bool,
}
