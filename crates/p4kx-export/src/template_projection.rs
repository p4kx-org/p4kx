// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_v3::tag_catalog::ContextualTagPath;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_context_value::{TemplateLocalization, TemplateVocabularyProjection};
use crate::template_hauling_projection::TemplateHaulingOrder;
use crate::template_input::TemplateInputs;
use crate::template_source_meaning::{ObjectiveBinding, PropertyBinding};
use crate::template_source_refs::{PhysicalTemplateRef, SourceFieldRef};

/// Permanent context computed from one physical Template's canonical source cells.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateProjection<'source> {
    pub(crate) template: PhysicalTemplateRef,
    pub(crate) inputs: TemplateInputs,
    pub(crate) title: Option<SourceValueRef>,
    #[serde(flatten)]
    pub(crate) context: MissionContext<'source>,
}

/// Contextual bindings and meaning shared by physical Template and Broker owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionContext<'source> {
    pub(crate) objectives: Vec<TemplateObjectiveProjection>,
    pub(crate) properties: Vec<crate::template_property_projection::TemplatePropertyProjection>,
    pub(crate) value_meanings: Vec<crate::template_value_meaning::TemplateValueMeaning>,
    pub(crate) localizations: Vec<TemplateLocalization>,
    pub(crate) tag_paths: Vec<TemplateTagProjection>,
    pub(crate) property_bindings: Vec<TemplatePropertyBinding>,
    pub(crate) objective_bindings: Vec<TemplateObjectiveTarget>,
    pub(crate) linked_outputs: Vec<TemplateOutputBinding>,
    pub(crate) vocabularies: Vec<TemplateVocabularyProjection>,
    pub(crate) modifiers: Vec<crate::template_modifier_projection::TemplateModifierProjection>,
    pub(crate) owner: Option<TemplateOwnerProjection<'source>>,
    pub(crate) hauling_orders: Vec<TemplateHaulingOrder>,
    pub(crate) spawn_pools: Vec<crate::template_spawn_projection::TemplateSpawnProjection>,
    pub(crate) pool_bindings: Vec<crate::template_spawn_projection::TemplatePoolBinding>,
}

/// The established preorder selection, identified physically instead of by an authored id.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateObjectiveProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) parent: Option<SourceValueRef>,
    pub(crate) id: SourceFieldRef,
}

/// The actual lookup decision independently of the original reference cell's state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum TemplateTagInterpretation {
    Path,
    GuidFallback,
    Inapplicable,
    Unresolved,
    Null,
    WrongShape,
    Unreadable,
    Unavailable,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateTagProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) interpretation: TemplateTagInterpretation,
    pub(crate) path: ContextualTagPath,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplatePropertyBinding {
    pub(crate) caller: SourceValueRef,
    pub(crate) target: PropertyBinding,
    pub(crate) mission_property_status:
        Option<SourcedValue<crate::scoped_location_link::PropertyReferenceStatus>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateOutputBinding {
    pub(crate) caller: SourceValueRef,
    pub(crate) variable: PropertyBinding,
    pub(crate) selected: bool,
    pub(crate) objective: ObjectiveBinding,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateOwnerProjection<'source> {
    pub(crate) source: SourceValueRef,
    pub(crate) class_name: SourcedValue<&'source str>,
}

/// The actual same-owner id-field selection of an original weak slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateObjectiveTarget {
    pub(crate) caller: SourceValueRef,
    pub(crate) target: ObjectiveBinding,
}
