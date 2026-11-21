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
pub struct ShipInsurance<'source> {
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
pub struct ShipInsuranceParams<'source> {
    pub(crate) ship_entity_class_name: SourcedValue<&'source str>,
    pub(crate) base_wait_time_minutes: SourcedValue<ResourceNumber>,
    pub(crate) mandatory_wait_time_minutes: SourcedValue<ResourceNumber>,
    pub(crate) expedite_fee: SourcedValue<ResourceNumber>,
    pub(crate) additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ShipInsurance<'source> {
    /// Borrows the original selected static-element position.
    #[must_use]
    pub const fn static_element_ordinal(&self) -> &SourcedValue<u32> {
        &self.static_element_ordinal
    }

    /// Borrows the authored claim-restoration flag.
    #[must_use]
    pub const fn restore_on_claim(&self) -> &SourcedValue<bool> {
        &self.restore_on_claim
    }

    /// Borrows the original insurance-parameter state.
    #[must_use]
    pub const fn params(&self) -> &SourcedValue<ShipInsuranceParams<'source>> {
        &self.params
    }

    /// Borrows other original insurance fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}

impl<'source> ShipInsuranceParams<'source> {
    /// Borrows the authored insured entity class name.
    #[must_use]
    pub const fn ship_entity_class_name(&self) -> &SourcedValue<&'source str> {
        &self.ship_entity_class_name
    }

    /// Borrows the exact base wait in minutes.
    #[must_use]
    pub const fn base_wait_time_minutes(&self) -> &SourcedValue<ResourceNumber> {
        &self.base_wait_time_minutes
    }

    /// Borrows the exact mandatory wait in minutes.
    #[must_use]
    pub const fn mandatory_wait_time_minutes(&self) -> &SourcedValue<ResourceNumber> {
        &self.mandatory_wait_time_minutes
    }

    /// Borrows the exact expedite fee in aUEC.
    #[must_use]
    pub const fn expedite_fee(&self) -> &SourcedValue<ResourceNumber> {
        &self.expedite_fee
    }

    /// Borrows other original parameter fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }
}
