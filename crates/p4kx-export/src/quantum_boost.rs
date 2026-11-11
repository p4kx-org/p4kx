// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// The authored boost configuration of one quantum drive.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct QuantumBoost<'source> {
    source_type: SourcedValue<&'source str>,
    max_boost_speed: SourcedValue<ResourceNumber>,
    time_to_max_boost_speed: SourcedValue<ResourceNumber>,
    boost_use_time: SourcedValue<ResourceNumber>,
    boost_recharge_time: SourcedValue<ResourceNumber>,
    stop_time: SourcedValue<ResourceNumber>,
    min_jump_distance: SourcedValue<ResourceNumber>,
    ifcs_handover_down_time: SourcedValue<ResourceNumber>,
    ifcs_handover_respool_time: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumBoost<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            max_boost_speed: fields.number("maxBoostSpeed")?,
            time_to_max_boost_speed: fields.number("timeToMaxBoostSpeed")?,
            boost_use_time: fields.number("boostUseTime")?,
            boost_recharge_time: fields.number("boostRechargeTime")?,
            stop_time: fields.number("stopTime")?,
            min_jump_distance: fields.number("minJumpDistance")?,
            ifcs_handover_down_time: fields.number("ifcsHandoverDownTime")?,
            ifcs_handover_respool_time: fields.number("ifcsHandoverRespoolTime")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}
