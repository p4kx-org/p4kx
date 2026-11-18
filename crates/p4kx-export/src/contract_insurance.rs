// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceArenaOutput;

/// The selected original static insurance declaration and its independent parameter state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipInsurance<'source> {
    pub(crate) static_element_ordinal: SourcedValue<u32>,
    pub(crate) restore_on_claim: SourcedValue<bool>,
    #[serde(rename = "shipInsuranceParams")]
    pub(crate) params: SourcedValue<ShipInsuranceParams<'source>>,
    pub(crate) additional_fields: SourceArenaOutput<'source>,
}

/// Exact original insurance parameters, with wait values in minutes and expedite fee in aUEC.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipInsuranceParams<'source> {
    pub(crate) ship_entity_class_name: SourcedValue<&'source str>,
    pub(crate) base_wait_time_minutes: SourcedValue<ResourceNumber>,
    pub(crate) mandatory_wait_time_minutes: SourcedValue<ResourceNumber>,
    pub(crate) expedite_fee: SourcedValue<ResourceNumber>,
    pub(crate) additional_fields: SourceArenaOutput<'source>,
}
