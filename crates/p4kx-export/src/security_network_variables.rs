// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::security_network_protocol::HostilityRules;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::interaction_source::{InteractionList, RecordLink, WeakLink};
use crate::catalog_v3::interaction_state_edges::STATE;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const VARIABLE: &str = "SecurityNetworkVariable";
const VALUE_FAMILY: &str = "SecurityNetworkVariableValue_Base";
const EFFECTS: &str = "SecurityNetworkVariableEffects";
const EFFECT_FAMILY: &str = "SecurityNetworkVariableEffect_Base";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SecurityNetworkVariable<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    tag: RecordLink<'source>,
    value: SourcedValue<SecurityVariableValue<'source>>,
}

impl<'source> SecurityNetworkVariable<'source> {
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<InteractionList<Self>, CatalogError> {
        fields.class_array_to("variables", VARIABLE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tag = fields.record_link_to("tag", "Tag", notes)?;
        let value = fields.strong_pointer_family_to(
            "value",
            VALUE_FAMILY,
            notes,
            SecurityVariableValue::read,
        )?;
        let source = UnitSource::read(fields, [("tag", tag.state()), ("value", value.state())])?;
        Ok(Self { source, tag, value })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SecurityVariableValue<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    value: VariableValue,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
enum VariableValue {
    Bool { value: SourcedValue<bool> },
    Int { value: SourcedValue<i32> },
    Authored,
}

impl VariableValue {
    const fn projected(&self) -> Option<(&'static str, SourceValueState)> {
        match self {
            Self::Bool { value } => Some(("value", value.state())),
            Self::Int { value } => Some(("value", value.state())),
            Self::Authored => None,
        }
    }
}

impl<'source> SecurityVariableValue<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let value = match fields.source_type()? {
            "SecurityNetworkVariableValue_Bool" => VariableValue::Bool {
                value: fields.boolean("value", notes)?,
            },
            "SecurityNetworkVariableValue_Int" => VariableValue::Int {
                value: fields.int32("value", notes)?,
            },
            _ => {
                fields.note_retained_variant(notes)?;
                VariableValue::Authored
            }
        };
        let source = UnitSource::read(fields, value.projected())?;
        Ok(Self { source, value })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SecurityVariableEffects<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    variable_value: SourcedValue<SecurityNetworkVariable<'source>>,
    effects: InteractionList<SecurityVariableEffect<'source>>,
}

impl<'source> SecurityVariableEffects<'source> {
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<InteractionList<Self>, CatalogError> {
        fields.class_array_to("variableEffects", EFFECTS, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let variable_value = fields.declared_class_to(
            "variableValue",
            VARIABLE,
            notes,
            SecurityNetworkVariable::read,
        )?;
        let effects = fields.strong_pointer_family_array_to(
            "effects",
            EFFECT_FAMILY,
            notes,
            SecurityVariableEffect::read,
        )?;
        let source = UnitSource::read(
            fields,
            [
                ("variableValue", variable_value.state()),
                ("effects", effects.state()),
            ],
        )?;
        Ok(Self {
            source,
            variable_value,
            effects,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SecurityVariableEffect<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    effect: VariableEffect<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
enum VariableEffect<'source> {
    SetInteractionState {
        state: WeakLink<'source>,
    },
    SetHostilityRules {
        rules: SourcedValue<Box<HostilityRules<'source>>>,
    },
    Authored,
}

impl VariableEffect<'_> {
    const fn projected(&self) -> Option<(&'static str, SourceValueState)> {
        match self {
            Self::SetInteractionState { state } => Some(("state", state.state())),
            Self::SetHostilityRules { rules } => Some(("rules", rules.state())),
            Self::Authored => None,
        }
    }
}

impl<'source> SecurityVariableEffect<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let effect = match fields.source_type()? {
            "SecurityNetworkVariableEffect_SetInteractionState" => {
                VariableEffect::SetInteractionState {
                    state: fields.weak_link_to("state", STATE, notes)?,
                }
            }
            "SecurityNetworkVariableEffect_SetHostilityRules" => {
                VariableEffect::SetHostilityRules {
                    rules: HostilityRules::read_effect(fields, notes)?.map(Box::new),
                }
            }
            _ => {
                fields.note_retained_variant(notes)?;
                VariableEffect::Authored
            }
        };
        let source = UnitSource::read(fields, effect.projected())?;
        Ok(Self { source, effect })
    }
}
