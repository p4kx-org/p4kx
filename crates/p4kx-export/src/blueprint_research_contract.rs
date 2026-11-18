// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// The declared research envelope and its supported interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintResearch {
    pub(crate) source: Option<SourceValueRef>,
    pub(crate) envelope: SourcedValue<BlueprintResearchEnvelope>,
}

/// Both actual slots of the recognized research envelope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintResearchEnvelope {
    pub(crate) research_costs: BlueprintResearchPointer,
    pub(crate) unlock_requirements: BlueprintResearchPointer,
}

/// The original pointer declaration, target base and support state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintResearchPointer {
    pub(crate) source: Option<SourceValueRef>,
    pub(crate) declared_base: SourcedValue<String>,
    pub(crate) value: SourcedValue<SourceValueRef>,
}

impl BlueprintResearch {
    /// Returns the declared research occurrence when available.
    #[must_use]
    pub const fn source(&self) -> Option<SourceValueRef> {
        self.source
    }

    /// Borrows the recognized envelope and its original value state.
    #[must_use]
    pub const fn envelope(&self) -> &SourcedValue<BlueprintResearchEnvelope> {
        &self.envelope
    }
}

impl BlueprintResearchEnvelope {
    /// Borrows the original research cost pointer interpretation.
    #[must_use]
    pub const fn research_costs(&self) -> &BlueprintResearchPointer {
        &self.research_costs
    }

    /// Borrows the original unlock requirement pointer interpretation.
    #[must_use]
    pub const fn unlock_requirements(&self) -> &BlueprintResearchPointer {
        &self.unlock_requirements
    }
}

impl BlueprintResearchPointer {
    /// Returns the original pointer occurrence when available.
    #[must_use]
    pub const fn source(&self) -> Option<SourceValueRef> {
        self.source
    }

    /// Borrows the original declared pointer target base.
    #[must_use]
    pub const fn declared_base(&self) -> &SourcedValue<String> {
        &self.declared_base
    }

    /// Borrows the interpreted target occurrence and its support state.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<SourceValueRef> {
        &self.value
    }
}
