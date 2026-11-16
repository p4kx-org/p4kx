// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::DecimalU64;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::{SourceField, SourceNode};
use crate::template_action_meaning::{ActionMeaning, RandomPhaseMeaning};
use crate::template_source_meaning::{ConditionMeaning, TriggerMeaning};

/// The single canonical raw inventory of one physical Template owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSource<'source> {
    pub(crate) roots: Vec<TemplateSourceRoot<'source>>,
    pub(crate) values: Vec<TemplateSourceValue<'source>>,
}

/// One actual original Field root and its coordinates before selection into this arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSourceRoot<'source> {
    pub(crate) source_position: OriginalFieldPosition,
    #[serde(flatten)]
    pub(crate) field: SourceField<'source>,
}

/// Original position controls, including an owning Struct that is not itself emitted.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct OriginalFieldPosition {
    pub(crate) parent_ordinal: Option<DecimalU64>,
    pub(crate) field_index: DecimalU64,
}

/// One exact original occurrence and its selected interpretation.
#[derive(Debug)]
pub(crate) struct TemplateSourceValue<'source> {
    pub(crate) source_ordinal: DecimalU64,
    pub(crate) original: SourceNode<'source>,
    pub(crate) meaning: TemplateValueMeaning,
}

/// The selected interpretation, if the original occurrence has a flow role.
#[derive(Debug, Default)]
pub(crate) enum TemplateValueMeaning {
    #[default]
    None,
    Condition(Box<ConditionMeaning>),
    Action(Box<ActionMeaning>),
    RandomPhase(Box<RandomPhaseMeaning>),
    Trigger(Box<TriggerMeaning>),
}

/// One exact original occurrence with separate nullable typed meanings.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "TemplateSourceValue", transform = every_field_required)]
struct WireValue<'read, 'source> {
    source_ordinal: DecimalU64,
    #[serde(flatten)]
    original: &'read SourceNode<'source>,
    condition: Option<&'read ConditionMeaning>,
    action: Option<&'read ActionMeaning>,
    random_phase: Option<&'read RandomPhaseMeaning>,
    trigger: Option<&'read TriggerMeaning>,
}

impl Serialize for TemplateSourceValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = WireValue {
            source_ordinal: self.source_ordinal,
            original: &self.original,
            condition: None,
            action: None,
            random_phase: None,
            trigger: None,
        };
        match &self.meaning {
            TemplateValueMeaning::None => {}
            TemplateValueMeaning::Condition(meaning) => value.condition = Some(meaning),
            TemplateValueMeaning::Action(meaning) => value.action = Some(meaning),
            TemplateValueMeaning::RandomPhase(meaning) => value.random_phase = Some(meaning),
            TemplateValueMeaning::Trigger(meaning) => value.trigger = Some(meaning),
        }
        value.serialize(serializer)
    }
}

impl JsonSchema for TemplateSourceValue<'_> {
    fn schema_name() -> Cow<'static, str> {
        WireValue::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        WireValue::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        WireValue::json_schema(generator)
    }
}

#[cfg(test)]
#[path = "template_source_tests.rs"]
mod tests;
