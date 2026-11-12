// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_source::{InteractionList, WeakLink};
use super::interaction_state::InteractionState;
use super::interaction_state_edges::STATE;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "SInteractionStateType";

/// An authored state category with ordered state and modifier slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionStateType<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    state_type_name: SourcedValue<&'source str>,
    bindings_method: SourcedValue<&'source str>,
    default_state: WeakLink<'source>,
    asop_reset: SourcedValue<bool>,
    network_authority: SourcedValue<&'source str>,
    states: InteractionList<InteractionState<'source>>,
    state_modifiers: InteractionList<InteractionStateModifier<'source>>,
}

impl<'source> InteractionStateType<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let state_type_name = source.borrowed_string("StateTypeName", notes)?;
        let bindings_method =
            source.borrowed_enum_to("bindingsMethod", "InteractionBindingsMethod", notes)?;
        let default_state = source.weak_link_to("DefaultState", STATE, notes)?;
        let asop_reset = source.boolean("asopReset", notes)?;
        let network_authority =
            source.borrowed_enum_to("networkAuthority", "StateTypeNetworkAuthority", notes)?;
        let states = source.strong_pointer_family_array_to(
            "States",
            STATE,
            notes,
            InteractionState::read,
        )?;
        let state_modifiers = source.strong_pointer_family_array_to(
            "StateModifiers",
            "SStateModifier",
            notes,
            InteractionStateModifier::read,
        )?;
        let source = UnitSource::read(
            source,
            [
                ("StateTypeName", state_type_name.state()),
                ("bindingsMethod", bindings_method.state()),
                ("DefaultState", default_state.state()),
                ("asopReset", asop_reset.state()),
                ("networkAuthority", network_authority.state()),
                ("States", states.state()),
                ("StateModifiers", state_modifiers.state()),
            ],
        )?;
        Ok(Self {
            source,
            state_type_name,
            bindings_method,
            default_state,
            asop_reset,
            network_authority,
            states,
            state_modifiers,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum InteractionStateModifier<'source> {
    CargoOccupancy(CargoOccupancyModifier<'source>),
    Original(UnitSource<'source>),
}

impl<'source> InteractionStateModifier<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "CargoOccupancyStateModifier" => {
                Self::CargoOccupancy(CargoOccupancyModifier::read(source, notes)?)
            }
            _ => Self::Original(UnitSource::read(source, [])?),
        })
    }
}

/// The authored state links for vacant and occupied storage.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CargoOccupancyModifier<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    cargo_vacant: WeakLink<'source>,
    cargo_stored: WeakLink<'source>,
}

impl<'source> CargoOccupancyModifier<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let cargo_vacant = source.weak_link_to("cargoVacant", STATE, notes)?;
        let cargo_stored = source.weak_link_to("cargoStored", STATE, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("cargoVacant", cargo_vacant.state()),
                ("cargoStored", cargo_stored.state()),
            ],
        )?;
        Ok(Self {
            source,
            cargo_vacant,
            cargo_stored,
        })
    }
}
