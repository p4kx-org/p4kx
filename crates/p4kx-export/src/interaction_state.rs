// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_point;
use super::interaction_source::{
    InteractionList, RecordLink, RecordLinks, WeakLink, WeakLinks, original,
};
use super::interaction_state_edges::{
    self, InteractionTagList, InteractionTransition, StateAutoChange,
};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// One physical interaction state and its authored effects and graph edges.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct InteractionState<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    state: StateFields<'source>,
    #[serde(flatten)]
    effects: StateEffects<'source>,
}

impl<'source> InteractionState<'source> {
    /// Borrows the original state name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.state.state_name
    }

    /// Borrows the original UI bindings Tag reference.
    #[must_use]
    pub const fn ui_bindings_tag(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.state.ui_bindings_tag
    }

    /// Borrows the authored state Tag list.
    #[must_use]
    pub const fn state_tags(&self) -> &SourcedValue<InteractionTagList<'source>> {
        &self.state.state_tags
    }

    /// Borrows the authored Entity Tag list.
    #[must_use]
    pub const fn entity_tags(&self) -> &SourcedValue<InteractionTagList<'source>> {
        &self.state.entity_tags
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let state = StateFields::read(source, notes)?;
        let effects = StateEffects::read(source, notes)?;
        let source =
            UnitSource::read(source, state.members().into_iter().chain(effects.members()))?;
        Ok(Self {
            source,
            state,
            effects,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct StateFields<'source> {
    state_name: SourcedValue<&'source str>,
    state_auto_change: SourcedValue<StateAutoChange<'source>>,
    interactions: InteractionList<InteractionTransition<'source>>,
    interaction_points: WeakLinks<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    ui_bindings_tag: RecordLink<'source>,
    state_tags: SourcedValue<InteractionTagList<'source>>,
    entity_tags: SourcedValue<InteractionTagList<'source>>,
}

impl<'source> StateFields<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            state_name: source.borrowed_string("StateName", notes)?,
            state_auto_change: source.strong_pointer_family_to(
                "StateAutoChange",
                "SStateAutoChange",
                notes,
                StateAutoChange::read,
            )?,
            interactions: source.class_array_to(
                "Interactions",
                interaction_state_edges::TRANSITION,
                notes,
                InteractionTransition::read,
            )?,
            interaction_points: source.weak_link_array_to(
                "interactionPoints",
                interaction_point::SOURCE_TYPE,
                notes,
            )?,
            ui_bindings_tag: source.record_link_to("uiBindingsTag", "Tag", notes)?,
            state_tags: source.declared_class_to(
                "stateTags",
                "TagList",
                notes,
                InteractionTagList::read,
            )?,
            entity_tags: source.declared_class_to(
                "entityTags",
                "TagList",
                notes,
                InteractionTagList::read,
            )?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("StateName", self.state_name.state()),
            ("StateAutoChange", self.state_auto_change.state()),
            ("Interactions", self.interactions.state()),
            ("interactionPoints", self.interaction_points.state()),
            ("uiBindingsTag", self.ui_bindings_tag.state()),
            ("stateTags", self.state_tags.state()),
            ("entityTags", self.entity_tags.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct StateEffects<'source> {
    enter_state_effect_group: SourcedValue<&'source str>,
    exit_state_effect_group: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    enter_state_effect_tags: RecordLinks<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    exit_state_effect_tags: RecordLinks<'source>,
    enter_event_dispatchers: InteractionList<UnitSource<'source>>,
    exit_event_dispatchers: InteractionList<UnitSource<'source>>,
    enter_state_play_sequence: WeakLink<'source>,
    exit_state_play_sequence: WeakLink<'source>,
    enter_geometry_tags: SourcedValue<&'source str>,
}

impl<'source> StateEffects<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            enter_state_effect_group: source.borrowed_string("EnterStateEffectGroup", notes)?,
            exit_state_effect_group: source.borrowed_string("ExitStateEffectGroup", notes)?,
            enter_state_effect_tags: source.record_link_array_to(
                "enterStateEffectTags",
                "Tag",
                notes,
            )?,
            exit_state_effect_tags: source.record_link_array_to(
                "exitStateEffectTags",
                "Tag",
                notes,
            )?,
            enter_event_dispatchers: source.strong_pointer_family_array_to(
                "EnterEventDispatchers",
                "EventDispatcher",
                notes,
                original,
            )?,
            exit_event_dispatchers: source.strong_pointer_family_array_to(
                "ExitEventDispatchers",
                "EventDispatcher",
                notes,
                original,
            )?,
            enter_state_play_sequence: source.weak_link_to(
                "EnterStatePlaySequence",
                "SSequencerDefSequenceParams",
                notes,
            )?,
            exit_state_play_sequence: source.weak_link_to(
                "ExitStatePlaySequence",
                "SSequencerDefSequenceParams",
                notes,
            )?,
            enter_geometry_tags: source.borrowed_string("enterGeometryTags", notes)?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 9] {
        [
            (
                "EnterStateEffectGroup",
                self.enter_state_effect_group.state(),
            ),
            ("ExitStateEffectGroup", self.exit_state_effect_group.state()),
            ("enterStateEffectTags", self.enter_state_effect_tags.state()),
            ("exitStateEffectTags", self.exit_state_effect_tags.state()),
            (
                "EnterEventDispatchers",
                self.enter_event_dispatchers.state(),
            ),
            ("ExitEventDispatchers", self.exit_event_dispatchers.state()),
            (
                "EnterStatePlaySequence",
                self.enter_state_play_sequence.state(),
            ),
            (
                "ExitStatePlaySequence",
                self.exit_state_play_sequence.state(),
            ),
            ("enterGeometryTags", self.enter_geometry_tags.state()),
        ]
    }
}
