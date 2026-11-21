// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// The original context and the actual effective cost rules.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintCostContext {
    pub(crate) source: Option<SourceValueRef>,
    pub(crate) quantity_multiplier: Option<SourcedValue<f32>>,
    pub(crate) composition_inclusion: Option<SourceValueRef>,
    pub(crate) modifiers: Vec<BlueprintModifier>,
}

/// An original property modifier and its known ordered ranges.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintModifier {
    pub(crate) source: SourceValueRef,
    pub(crate) property_guid: SourcedValue<CatalogGuid>,
    pub(crate) ranges: Vec<BlueprintModifierRange>,
}

/// The exact original range and its known finite interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintModifierRange {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: BlueprintRangeKind,
    pub(crate) start_quality: SourcedValue<ResourceInt64>,
    pub(crate) end_quality: SourcedValue<ResourceInt64>,
    pub(crate) at_start: SourcedValue<f32>,
    pub(crate) at_end: SourcedValue<f32>,
}

/// The supported gameplay property range interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum BlueprintRangeKind {
    /// A multiplicative contribution relative to one.
    Multiplier,
    /// An additive contribution to the original base value.
    Additive,
}

impl BlueprintCostContext {
    /// Returns the original context occurrence when available.
    #[must_use]
    pub const fn source(&self) -> Option<SourceValueRef> {
        self.source
    }

    /// Borrows the effective quantity multiplier when interpreted.
    #[must_use]
    pub const fn quantity_multiplier(&self) -> Option<&SourcedValue<f32>> {
        self.quantity_multiplier.as_ref()
    }

    /// Returns the effective original composition option occurrence.
    #[must_use]
    pub const fn composition_inclusion(&self) -> Option<SourceValueRef> {
        self.composition_inclusion
    }

    /// Borrows original property modifiers in authored order.
    #[must_use]
    pub fn modifiers(&self) -> &[BlueprintModifier] {
        &self.modifiers
    }
}

impl BlueprintModifier {
    /// Returns this modifier occurrence within its Blueprint arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// Borrows the original gameplay property GUID interpretation.
    #[must_use]
    pub const fn property_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.property_guid
    }

    /// Borrows recognized original ranges in authored order.
    #[must_use]
    pub fn ranges(&self) -> &[BlueprintModifierRange] {
        &self.ranges
    }
}

impl BlueprintModifierRange {
    /// Returns this range occurrence within its Blueprint arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// Borrows the supported modifier operation.
    #[must_use]
    pub const fn kind(&self) -> &BlueprintRangeKind {
        &self.kind
    }

    /// Borrows the interpreted starting quality bound.
    #[must_use]
    pub const fn start_quality(&self) -> &SourcedValue<ResourceInt64> {
        &self.start_quality
    }

    /// Borrows the interpreted ending quality bound.
    #[must_use]
    pub const fn end_quality(&self) -> &SourcedValue<ResourceInt64> {
        &self.end_quality
    }

    /// Borrows the finite binary32 projection at the starting quality.
    #[must_use]
    pub const fn at_start(&self) -> &SourcedValue<f32> {
        &self.at_start
    }

    /// Borrows the finite binary32 projection at the ending quality.
    #[must_use]
    pub const fn at_end(&self) -> &SourcedValue<f32> {
        &self.at_end
    }
}
