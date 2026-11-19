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
#[non_exhaustive]
pub struct ResourceState<'source> {
    /// Original DataForge declaration.
    pub source_type: SourcedValue<&'source str>,
    /// Zero-based position in the original array.
    pub source_position: SourcedValue<u32>,
    /// Authored state name.
    pub name: SourcedValue<&'source str>,
    /// Ordered authored resource deltas.
    pub deltas: SourcedValue<Vec<SourcedValue<ResourceDelta<'source>>>>,
    /// Ordered authored interaction links.
    pub linked_interaction_states: SourcedValue<Vec<SourcedValue<LinkedInteraction<'source>>>>,
    /// Authored electromagnetic and infrared signatures.
    pub signature_params: SourcedValue<ResourceSignatureParams<'source>>,
    /// Authored power ranges.
    pub power_ranges: SourcedValue<ResourcePowerRanges<'source>>,
    /// Original fields outside the typed projection.
    pub additional_fields: SourceArenaOutput<'source>,
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

/// Original interaction-state link and change flag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LinkedInteraction<'source> {
    /// Original DataForge declaration.
    pub source_type: SourcedValue<&'source str>,
    #[serde(rename = "linkedInteractionState")]
    /// Original reference to the interaction state.
    pub state: SourcedValue<HealthLink<'source>>,
    /// Whether a state change is requested.
    pub change_interaction_state: SourcedValue<bool>,
    /// Original fields outside the typed projection.
    pub additional_fields: SourceArenaOutput<'source>,
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
