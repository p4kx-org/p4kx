// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::authored_link::HealthLink;
use super::catalog_resource_delta::ResourceDelta;
use super::catalog_resource_power_ranges::ResourcePowerRanges;
use super::catalog_resource_signature::ResourceSignatureParams;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// The authored configuration and ordered deltas of one resource state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceState<'source> {
    source_type: SourcedValue<&'source str>,
    source_position: SourcedValue<u32>,
    name: SourcedValue<&'source str>,
    deltas: SourcedValue<Vec<SourcedValue<ResourceDelta<'source>>>>,
    linked_interaction_states: SourcedValue<Vec<SourcedValue<LinkedInteraction<'source>>>>,
    signature_params: SourcedValue<ResourceSignatureParams<'source>>,
    power_ranges: SourcedValue<ResourcePowerRanges<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceState<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = source.borrowed_string("name", notes)?;
        let deltas = source.declared_family_array("deltas", notes, ResourceDelta::read)?;
        let linked_interaction_states = source.declared_class_array(
            "linkedInteractionStates",
            notes,
            LinkedInteraction::read,
        )?;
        let signature_params =
            source.declared_class("signatureParams", notes, ResourceSignatureParams::read)?;
        let power_ranges =
            source.declared_class("powerRanges", notes, ResourcePowerRanges::read)?;
        let additional_fields = source.authored_members_except([
            ("name", name.state()),
            ("deltas", deltas.state()),
            ("linkedInteractionStates", linked_interaction_states.state()),
            ("signatureParams", signature_params.state()),
            ("powerRanges", power_ranges.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            source_position: source.array_ordinal()?,
            name,
            deltas,
            linked_interaction_states,
            signature_params,
            power_ranges,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LinkedInteraction<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(rename = "linkedInteractionState")]
    state: SourcedValue<HealthLink<'source>>,
    change_interaction_state: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> LinkedInteraction<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let state = source.link("linkedInteractionState", notes)?;
        let change_interaction_state = source.boolean("changeInteractionState", notes)?;
        let additional_fields = source.authored_members_except([
            ("linkedInteractionState", state.state()),
            ("changeInteractionState", change_interaction_state.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            state,
            change_interaction_state,
            additional_fields,
        })
    }
}
