// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const SOURCE_TYPE: &str = "SQedVisualGraphParams";
const STATE_PARAMS: &str = "SQedVisualGraphStateParams";
const ANIMATION_PARAMS: &str = "SQedVisualGraphAnimationParams";
const EFFECT_PARAMS: &str = "SQedVisualGraphEffectParams";
const TRANSITION_PARAMS: &str = "SQedVisualGraphTransitionParams";
const SELECTOR_PARAMS: &str = "SQedVisualGraphTransitionTypeParams";
const STATE_ENUM: &str = "EQedVisualGraphState";
const CONTEXT_ENUM: &str = "EQedVisualGraphTrackedVariableContext";
const VARIABLE_ENUM: &str = "EQedVisualGraphTrackedVariable";
const TAG: &str = "Tag";
const TIME_BASED: &str = "SQedVisualGraphTransitionTypeParams_TimeBased";
const VALUE_BASED: &str = "SQedVisualGraphTransitionTypeParams_ValueBased";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type OrderedCells<T> = SourcedValue<Vec<SourcedValue<T>>>;

/// Authored visual states, transitions, animation and effect parameters of a QIG.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct QuantumInterdictionVisualGraph<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    effect_strength_link: RecordLink<'source>,
    states: OrderedCells<QuantumInterdictionVisualState<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionVisualGraph<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            effect_strength_link: fields.record_link_to("effectStrengthLink", TAG)?,
            states: fields.project("states", |source, name, notes| {
                source.class_array_to(
                    name,
                    STATE_PARAMS,
                    notes,
                    QuantumInterdictionVisualState::read,
                )
            })?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumInterdictionVisualState<'source> {
    source_type: SourcedValue<&'source str>,
    animation_params: SourcedValue<QuantumInterdictionVisualAnimation<'source>>,
    effect_params: SourcedValue<QuantumInterdictionVisualEffect<'source>>,
    transitions: OrderedCells<QuantumInterdictionVisualTransition<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionVisualState<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            animation_params: fields.project("animationParams", |source, name, notes| {
                source.declared_class_to(
                    name,
                    ANIMATION_PARAMS,
                    notes,
                    QuantumInterdictionVisualAnimation::read,
                )
            })?,
            effect_params: fields.project("effectParams", |source, name, notes| {
                source.declared_class_to(
                    name,
                    EFFECT_PARAMS,
                    notes,
                    QuantumInterdictionVisualEffect::read,
                )
            })?,
            transitions: fields.project("transitions", |source, name, notes| {
                source.class_array_to(
                    name,
                    TRANSITION_PARAMS,
                    notes,
                    QuantumInterdictionVisualTransition::read,
                )
            })?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumInterdictionVisualAnimation<'source> {
    source_type: SourcedValue<&'source str>,
    fragment_name: SourcedValue<&'source str>,
    fragment_tag: SourcedValue<&'source str>,
    #[serde(rename = "loop")]
    loop_animation: SourcedValue<bool>,
    playback_bias: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionVisualAnimation<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            fragment_name: fields.project("fragmentName", |source, name, notes| {
                source.borrowed_string(name, notes)
            })?,
            fragment_tag: fields.project("fragmentTag", |source, name, notes| {
                source.borrowed_string(name, notes)
            })?,
            loop_animation: fields
                .project("loop", |source, name, notes| source.boolean(name, notes))?,
            playback_bias: fields.number("playbackBias")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumInterdictionVisualEffect<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    entity_effect_tag: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    entity_effect_trigger: RecordLink<'source>,
    strength: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionVisualEffect<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            entity_effect_tag: fields.record_link_to("entityEffectTag", TAG)?,
            entity_effect_trigger: fields.record_link_to("entityEffectTrigger", TAG)?,
            strength: fields.number("strength")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumInterdictionVisualTransition<'source> {
    source_type: SourcedValue<&'source str>,
    state: SourcedValue<&'source str>,
    animation_transition_type: SourcedValue<QuantumInterdictionTransitionSelector<'source>>,
    effect_transition_type: SourcedValue<QuantumInterdictionTransitionSelector<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionVisualTransition<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            state: fields.project("state", |source, name, notes| {
                source.borrowed_enum_to(name, STATE_ENUM, notes)
            })?,
            animation_transition_type: fields.project("animationTransitionType", read_selector)?,
            effect_transition_type: fields.project("effectTransitionType", read_selector)?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

fn read_selector<'source>(
    source: Observed<'_, '_, '_, 'source>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<QuantumInterdictionTransitionSelector<'source>>, CatalogError> {
    source.strong_pointer_family_to(
        name,
        SELECTOR_PARAMS,
        notes,
        QuantumInterdictionTransitionSelector::read,
    )
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumInterdictionTransitionSelector<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    parameters: QuantumInterdictionTransitionParameters<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionTransitionSelector<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type()?;
        if !matches!(source_type, TIME_BASED | VALUE_BASED) {
            source.note_retained_variant(notes)?;
        }
        let mut fields = QuantumFields::new(source, notes);
        let parameters = match source_type {
            TIME_BASED => QuantumInterdictionTransitionParameters::TimeBased {
                seconds: fields.number("seconds")?,
            },
            VALUE_BASED => QuantumInterdictionTransitionParameters::ValueBased {
                context: fields.project("context", |source, name, notes| {
                    source.borrowed_enum_to(name, CONTEXT_ENUM, notes)
                })?,
                variable: fields.project("variable", |source, name, notes| {
                    source.borrowed_enum_to(name, VARIABLE_ENUM, notes)
                })?,
            },
            _ => QuantumInterdictionTransitionParameters::Authored,
        };
        Ok(Self {
            source_type: fields.source_type()?,
            parameters,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum QuantumInterdictionTransitionParameters<'source> {
    TimeBased {
        seconds: SourcedValue<ResourceNumber>,
    },
    ValueBased {
        context: SourcedValue<&'source str>,
        variable: SourcedValue<&'source str>,
    },
    Authored,
}
