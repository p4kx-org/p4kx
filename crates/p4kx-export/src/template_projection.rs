// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_v3::tag_catalog::ContextualTagPath;
use crate::location_hauling_targets::HaulingTargetBinding;
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
pub struct TemplateProjection<'source> {
    pub(crate) template: PhysicalTemplateRef,
    pub(crate) inputs: TemplateInputs,
    pub(crate) title: Option<SourceValueRef>,
    #[serde(flatten)]
    pub(crate) context: MissionContext<'source>,
}

impl TemplateProjection<'_> {
    /// Links the original physical Template and its shared arena.
    #[must_use]
    pub const fn template(&self) -> &PhysicalTemplateRef {
        &self.template
    }

    /// Links the original title source value, when present.
    #[must_use]
    pub const fn title(&self) -> Option<SourceValueRef> {
        self.title
    }

    /// Borrows the canonical contextual objective selection.
    #[must_use]
    pub fn objectives(&self) -> &[TemplateObjectiveProjection] {
        &self.context.objectives
    }

    /// Borrows contextual selections that interpret original Tag references.
    #[must_use]
    pub fn tag_paths(&self) -> &[TemplateTagProjection] {
        &self.context.tag_paths
    }

    /// Borrows the original localization links and their lookup states.
    #[must_use]
    pub fn localizations(&self) -> &[TemplateLocalization] {
        &self.context.localizations
    }
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
    pub(crate) hauling_target_bindings: Vec<HaulingTargetBinding>,
    pub(crate) spawn_pools: Vec<crate::template_spawn_projection::TemplateSpawnProjection>,
    pub(crate) pool_bindings: Vec<crate::template_spawn_projection::TemplatePoolBinding>,
}

/// The established preorder selection, identified physically instead of by an authored id.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct TemplateObjectiveProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) parent: Option<SourceValueRef>,
    pub(crate) id: SourceFieldRef,
}

impl TemplateObjectiveProjection {
    /// Links this original objective into the owner arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// Links the containing objective when nested.
    #[must_use]
    pub const fn parent(&self) -> Option<SourceValueRef> {
        self.parent
    }

    /// Links the original objective ID declaration.
    #[must_use]
    pub const fn id(&self) -> SourceFieldRef {
        self.id
    }
}

/// The actual lookup decision independently of the original reference cell's state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[schemars(transform = crate::schema_fields::compact_string_enum)]
#[non_exhaustive]
pub enum TemplateTagInterpretation {
    /// The original reference resolves to a selected Tag path.
    #[schemars(description = "")]
    Path,
    /// The Tag is absent from a complete inventory and the GUID is retained.
    #[schemars(description = "")]
    GuidFallback,
    /// The original reference is an owner-local location.
    #[schemars(description = "")]
    Inapplicable,
    /// The original Tag reference is dangling.
    #[schemars(description = "")]
    Unresolved,
    /// The original occurrence is null.
    #[schemars(description = "")]
    Null,
    /// The original occurrence is another value shape.
    #[schemars(description = "")]
    WrongShape,
    /// The original occurrence cannot be decoded.
    #[schemars(description = "")]
    Unreadable,
    /// The Tag inventory cannot prove the requested absence.
    #[schemars(description = "")]
    Unavailable,
}

/// A contextual interpretation of an original Template Tag reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(description = "", transform = every_field_required)]
pub struct TemplateTagProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) interpretation: TemplateTagInterpretation,
    pub(crate) path: ContextualTagPath,
}

impl TemplateTagProjection {
    /// Links the owning Template's original Tag operand.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// Borrows the result of interpreting that operand.
    #[must_use]
    pub const fn interpretation(&self) -> &TemplateTagInterpretation {
        &self.interpretation
    }

    /// Returns the sourced path and its actual original ancestry.
    ///
    /// # Errors
    /// Returns a source accounting error if the lineage cannot be reconstructed.
    pub fn path(
        &self,
    ) -> Result<
        std::borrow::Cow<'_, SourcedValue<crate::catalog_v3::tag_catalog::OriginalTagPath>>,
        crate::CatalogError,
    > {
        self.path.sourced()
    }
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
