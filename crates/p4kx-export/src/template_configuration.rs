// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::mission_flow_owner::MissionFlowOwnerRef;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_refs::{DeclarationBinding, SourceFieldRef};

/// One actual root Flow declaration or its genuinely absent declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionFlowConfiguration {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) declaration_index: Option<DecimalU64>,
    pub(crate) declaration: DeclarationBinding,
    pub(crate) selection: FlowSelection,
    pub(crate) triggers: Vec<SourceValueRef>,
    pub(crate) referenced_fields: Vec<SourceFieldRef>,
}

/// The preserved first-declaration/first-Struct source selection recipe.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum FlowSelection {
    Absent,
    ShadowedDeclaration,
    NoStruct,
    Selected { value: SourceValueRef },
}

/// A typed link to one Flow declaration of its physical mission owner.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionFlowRef {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) declaration_index: Option<DecimalU64>,
}

/// One original mission or objective property declaration without a second raw arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionPropertyConfiguration {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) owner_objective: Option<SourceValueRef>,
    pub(crate) declaration_index: Option<DecimalU64>,
    pub(crate) declaration: DeclarationBinding,
    pub(crate) selection: PropertySelection,
    pub(crate) properties: Vec<SourceValueRef>,
}

impl MissionPropertyConfiguration {
    /// Borrows the actual physical owner of the declaration.
    #[must_use]
    pub const fn owner(&self) -> &MissionFlowOwnerRef {
        &self.owner
    }

    /// Returns the original objective's arena position, when there is one.
    #[must_use]
    pub const fn owner_objective(&self) -> Option<SourceValueRef> {
        self.owner_objective
    }

    /// Returns the field ordinal of the actual declaration, when present.
    #[must_use]
    pub fn declaration_index(&self) -> Option<u64> {
        self.declaration_index.map(DecimalU64::get)
    }

    /// Borrows the original declaration binding or proof of absence.
    #[must_use]
    pub const fn declaration(&self) -> &DeclarationBinding {
        &self.declaration
    }

    /// Borrows the original declaration's selection outcome.
    #[must_use]
    pub const fn selection(&self) -> &PropertySelection {
        &self.selection
    }

    /// Borrows the ordered original property occurrences in the owner's arena.
    #[must_use]
    pub fn properties(&self) -> &[SourceValueRef] {
        &self.properties
    }
}

/// The original declaration and objective selection independently of retained source facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum PropertySelection {
    /// The owner has no property declaration at this location.
    Absent,
    /// A preceding declaration is selected.
    ShadowedDeclaration,
    /// The declaration belongs to an objective that was outside the original selection.
    ShadowedObjective,
    /// The selected declaration contains no property structure.
    NoStruct,
    /// The selected declaration contains one or more property structures.
    Selected,
}

/// A typed link to one original property declaration of its physical owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionPropertyRef {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) owner_objective: Option<SourceValueRef>,
    pub(crate) declaration_index: Option<DecimalU64>,
}
