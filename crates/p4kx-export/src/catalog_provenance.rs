// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_extraction::SourceInventory;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::{CatalogError, CatalogSummary};

/// Immutable source inventory belonging to one catalog result or normal refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProvenance {
    inventory: Arc<SourceInventory>,
}

impl CatalogProvenance {
    pub(crate) fn new(inventory: SourceInventory) -> Self {
        Self {
            inventory: Arc::new(inventory),
        }
    }

    /// Borrows the exact immutable inventory captured for this result.
    #[must_use]
    pub fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}

impl Serialize for CatalogProvenance {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.inventory.as_ref().serialize(serializer)
    }
}

impl JsonSchema for CatalogProvenance {
    fn inline_schema() -> bool {
        SourceInventory::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        SourceInventory::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        SourceInventory::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SourceInventory::json_schema(generator)
    }
}

impl CatalogError {
    /// Borrows the immutable source inventory of an actual normal-publication refusal.
    #[must_use]
    pub fn source_inventory(&self) -> Option<&SourceInventory> {
        match self {
            Self::IncompleteExport(refusal) => Some(refusal.source_inventory()),
            Self::SourceEvidence(_)
            | Self::SourceUseAllocation(_)
            | Self::EntryPayloadAllocation { .. }
            | Self::JsonSourceAllocation(_)
            | Self::MissionTextLength
            | Self::MissionTextAllocation(_)
            | Self::MissionTextPlan
            | Self::LoadoutSourceEvidence(_)
            | Self::SourceOriginMismatch
            | Self::CleanupFailed { .. }
            | Self::Accounting(_)
            | Self::Load(_)
            | Self::DataForge(_)
            | Self::LoadTexts(_)
            | Self::Texts { .. }
            | Self::StoreSlugs(_)
            | Self::Json(_)
            | Self::Write(_)
            | Self::Cancelled => None,
        }
    }
}

impl CatalogSummary {
    /// Borrows the immutable source inventory of this actual catalog result.
    #[must_use]
    pub fn source_inventory(&self) -> &SourceInventory {
        self.provenance.inventory()
    }
}
