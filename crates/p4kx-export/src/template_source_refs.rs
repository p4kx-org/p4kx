// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, FieldOrigin, PhysicalSourceRef};
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// Structural navigation to one emitted physical Template owner.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct PhysicalTemplateRef {
    pub(crate) input_ref: PhysicalSourceRef,
    pub(crate) record_index: u32,
}

impl PhysicalTemplateRef {
    /// Identifies the decoded DataForge Entry that owns this Template.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Returns the original Record position in the owning DataForge Entry.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }
}

/// A source field stored once at an arena root or in an original Struct.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum SourceFieldRef {
    /// An original root declaration position.
    Root {
        /// The root field's original position.
        root_field_index: DecimalU64,
    },
    /// An original Struct member position.
    Field {
        /// The owning Struct occurrence.
        parent: SourceValueRef,
        /// The member's original declaration position.
        field_index: DecimalU64,
    },
}

/// Actual declaration presence or its original declaration-survey evidence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum DeclarationBinding {
    /// The original declaration is present.
    Present {
        /// The original field binding.
        field: SourceFieldRef,
    },
    /// The original owner has no such declaration.
    Absent {
        #[serde(flatten)]
        /// The original declaration-survey evidence.
        origin: FieldOrigin,
    },
}

/// A named member of the actual Struct whose meaning contains this binding.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum FlowFieldBinding {
    Field {
        field_index: DecimalU64,
    },
    Absent {
        #[serde(flatten)]
        origin: FieldOrigin,
    },
}

impl SourceFieldRef {
    /// Resolves this arena-local binding against its original owner.
    #[must_use]
    pub fn resolve<'arena, 'source>(
        self,
        arena: &'arena crate::source_value_catalog::SourceArenaOutput<'source>,
    ) -> Option<&'arena crate::source_value_catalog::SourceField<'source>> {
        self.resolve_using(
            |index| arena.roots().get(index),
            |parent| arena.value(parent),
        )
    }

    pub(crate) fn resolve_using<'arena, 'source: 'arena>(
        self,
        root: impl FnOnce(usize) -> Option<&'arena crate::source_value_catalog::SourceField<'source>>,
        value: impl FnOnce(
            SourceValueRef,
        ) -> Option<&'arena crate::source_value_catalog::SourceNode<'source>>,
    ) -> Option<&'arena crate::source_value_catalog::SourceField<'source>> {
        use crate::source_value_catalog::SourcePayload;

        match self {
            Self::Root { root_field_index } => root(usize::try_from(root_field_index.get()).ok()?),
            Self::Field {
                parent,
                field_index,
            } => {
                let SourcePayload::Struct { fields, .. } = value(parent)?.value().value()? else {
                    return None;
                };
                fields.get(usize::try_from(field_index.get()).ok()?)
            }
        }
    }
}
