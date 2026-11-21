// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::{
    QuantumInterdictionTransitionParameters, QuantumInterdictionTransitionSelector,
    QuantumInterdictionVisualAnimation, QuantumInterdictionVisualEffect,
    QuantumInterdictionVisualGraph, QuantumInterdictionVisualState,
    QuantumInterdictionVisualTransition,
};
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::resource_number::ResourceNumber;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

impl<'source> QuantumInterdictionVisualGraph<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The authored effect-strength Record reference.
        effect_strength_link: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// The original visual-state slots in authored order.
        states: SourcedValue<Vec<SourcedValue<QuantumInterdictionVisualState<'source>>>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumInterdictionVisualState<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The original animation settings.
        animation_params: SourcedValue<QuantumInterdictionVisualAnimation<'source>>,
        /// The original visual effect settings.
        effect_params: SourcedValue<QuantumInterdictionVisualEffect<'source>>,
        /// The original transitions in authored order.
        transitions: SourcedValue<Vec<SourcedValue<QuantumInterdictionVisualTransition<'source>>>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumInterdictionVisualAnimation<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The original animation fragment name.
        fragment_name: SourcedValue<&'source str>,
        /// The original animation fragment tag.
        fragment_tag: SourcedValue<&'source str>,
        /// The original animation loop flag.
        loop_animation: SourcedValue<bool>,
        /// The authored animation playback bias.
        playback_bias: SourcedValue<ResourceNumber>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumInterdictionVisualEffect<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The authored entity effect tag reference.
        entity_effect_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// The authored entity effect trigger reference.
        entity_effect_trigger: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// The authored effect strength.
        strength: SourcedValue<ResourceNumber>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumInterdictionVisualTransition<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The original transition state token.
        state: SourcedValue<&'source str>,
        /// The original animation transition selector.
        animation_transition_type: SourcedValue<QuantumInterdictionTransitionSelector<'source>>,
        /// The original effect transition selector.
        effect_transition_type: SourcedValue<QuantumInterdictionTransitionSelector<'source>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumInterdictionTransitionSelector<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The parameters of the original selector variant.
        parameters: QuantumInterdictionTransitionParameters<'source>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}
