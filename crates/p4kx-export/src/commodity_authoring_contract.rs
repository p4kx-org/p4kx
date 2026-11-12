// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::schema_fields::every_field_required;

/// Ordered commodity-owned bindings without presentation fallback.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommodityAuthoring<'source> {
    pub(crate) purchasable_parameters:
        SourcedValue<Vec<SourcedValue<CommodityPurchasable<'source>>>>,
    pub(crate) display_parameters: SourcedValue<Vec<SourcedValue<CommodityDisplay<'source>>>>,
    pub(crate) resource_containers:
        SourcedValue<Vec<SourcedValue<CommodityContainerLink<'source>>>>,
}

/// One original purchasable component with independently sourced text declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommodityPurchasable<'source> {
    pub(crate) source_position: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) display_name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_type: SourcedValue<GameTextRef>,
}

/// One original display component with its own description declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommodityDisplay<'source> {
    pub(crate) source_position: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_description: SourcedValue<GameTextRef>,
}

/// A physical link to the `ResourceContainer` configuration of this Commodity Entity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CommodityContainerLink<'source> {
    pub(crate) owner_record_index: SourcedValue<u32>,
    pub(crate) source_position: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
}
