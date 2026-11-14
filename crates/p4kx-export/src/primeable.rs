// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_link::HealthLink;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceText};

pub(super) const SOURCE_TYPE: &str = "SPrimeableComponentParams";

/// Authored priming controls and state links for one original component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Primeable<'source> {
    additional_fields: SourceArenaOutput<'source>,
    #[serde(flatten)]
    flags: PrimeFlags,
    #[serde(flatten)]
    states: PrimeStates<'source>,
    #[serde(rename = "primeableAnimatedStates")]
    animated_states: SourcedValue<Vec<SourcedValue<AnimatedPrimeState<'source>>>>,
}

impl<'source> Primeable<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let flags = PrimeFlags::read(source, notes)?;
        let states = PrimeStates::read(source, notes)?;
        let animated_states = source.declared_class_array(
            "primeableAnimatedStates",
            notes,
            AnimatedPrimeState::read,
        )?;
        let additional_fields = source.authored_members_except(
            flags
                .projected()
                .into_iter()
                .chain(states.projected())
                .chain([("primeableAnimatedStates", animated_states.state())]),
        )?;
        Ok(Self {
            additional_fields,
            flags,
            states,
            animated_states,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PrimeFlags {
    prime_on_attach: SourcedValue<bool>,
    prime_on_place: SourcedValue<bool>,
    prime_on_place_attach: SourcedValue<bool>,
    prime_on_take: SourcedValue<bool>,
    prime_on_throw: SourcedValue<bool>,
    unprime_on_take: SourcedValue<bool>,
}

impl PrimeFlags {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            prime_on_attach: source.boolean("primeOnAttach", notes)?,
            prime_on_place: source.boolean("primeOnPlace", notes)?,
            prime_on_place_attach: source.boolean("primeOnPlaceAttach", notes)?,
            prime_on_take: source.boolean("primeOnTake", notes)?,
            prime_on_throw: source.boolean("primeOnThrow", notes)?,
            unprime_on_take: source.boolean("unprimeOnTake", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("primeOnAttach", self.prime_on_attach.state()),
            ("primeOnPlace", self.prime_on_place.state()),
            ("primeOnPlaceAttach", self.prime_on_place_attach.state()),
            ("primeOnTake", self.prime_on_take.state()),
            ("primeOnThrow", self.prime_on_throw.state()),
            ("unprimeOnTake", self.unprime_on_take.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PrimeStates<'source> {
    on_prime_interaction: SourcedValue<HealthLink<'source>>,
    on_unprime_interaction: SourcedValue<HealthLink<'source>>,
    picked_up_state: SourcedValue<HealthLink<'source>>,
    primed_state: SourcedValue<HealthLink<'source>>,
    settled_state: SourcedValue<HealthLink<'source>>,
    unprimed_state: SourcedValue<HealthLink<'source>>,
}

impl<'source> PrimeStates<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            on_prime_interaction: source.link("onPrimeInteraction", notes)?,
            on_unprime_interaction: source.link("onUnprimeInteraction", notes)?,
            picked_up_state: source.link("pickedUpState", notes)?,
            primed_state: source.link("primedState", notes)?,
            settled_state: source.link("settledState", notes)?,
            unprimed_state: source.link("unprimedState", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("onPrimeInteraction", self.on_prime_interaction.state()),
            ("onUnprimeInteraction", self.on_unprime_interaction.state()),
            ("pickedUpState", self.picked_up_state.state()),
            ("primedState", self.primed_state.state()),
            ("settledState", self.settled_state.state()),
            ("unprimedState", self.unprimed_state.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AnimatedPrimeState<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    animated_state: SourcedValue<HealthLink<'source>>,
    fragment_id: SourcedValue<SourceText<'source>>,
    frag_tag: SourcedValue<SourceText<'source>>,
}

impl<'source> AnimatedPrimeState<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type()?;
        let animated_state = source.link("animatedState", notes)?;
        let fragment_id = source
            .borrowed_string("fragmentId", notes)?
            .map(|text| SourceText::read(source_type, "fragmentId", text));
        let frag_tag = source
            .borrowed_string("fragTag", notes)?
            .map(|text| SourceText::read(source_type, "fragTag", text));
        let source = UnitSource::read(
            source,
            [
                ("animatedState", animated_state.state()),
                ("fragmentId", fragment_id.state()),
                ("fragTag", frag_tag.state()),
            ],
        )?;
        Ok(Self {
            source,
            animated_state,
            fragment_id,
            frag_tag,
        })
    }
}
