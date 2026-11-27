// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_extraction::SourceInventory;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::variant_use_sets::VariantSourceUseSets;
use crate::{CatalogError, CatalogSummary};

/// Immutable source inventory belonging to one catalog result or normal refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProvenance {
    inventory: Arc<SourceInventory>,
    variant_source_use_sets: VariantSourceUseSets,
}

/// Borrowed canonical provenance for one result and its variant use sets.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvenanceView<'source> {
    #[serde(flatten)]
    inventory: &'source SourceInventory,
    #[serde(skip_serializing_if = "VariantSourceUseSets::is_empty")]
    variant_source_use_sets: &'source VariantSourceUseSets,
}

impl<'source> CatalogProvenanceView<'source> {
    pub(crate) const fn new(
        inventory: &'source SourceInventory,
        variant_source_use_sets: &'source VariantSourceUseSets,
    ) -> Self {
        Self {
            inventory,
            variant_source_use_sets,
        }
    }

    /// Borrows the original source inventory of this result.
    #[must_use]
    pub const fn inventory(&self) -> &'source SourceInventory {
        self.inventory
    }

    /// Borrows the shared use sets referenced by this result's variant decisions.
    #[must_use]
    pub const fn variant_source_use_sets(&self) -> &'source VariantSourceUseSets {
        self.variant_source_use_sets
    }
}

impl CatalogProvenance {
    pub(crate) fn new(inventory: SourceInventory) -> Self {
        Self {
            inventory: Arc::new(inventory),
            variant_source_use_sets: VariantSourceUseSets::default(),
        }
    }

    pub(crate) fn with_variant_source_use_sets(mut self, sets: VariantSourceUseSets) -> Self {
        self.variant_source_use_sets = sets;
        self
    }

    /// Borrows the exact immutable inventory captured for this result.
    #[must_use]
    pub fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    /// Borrows the shared use sets referenced by emitted variant decisions.
    #[must_use]
    pub const fn variant_source_use_sets(&self) -> &VariantSourceUseSets {
        &self.variant_source_use_sets
    }
}

impl Serialize for CatalogProvenance {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        CatalogProvenanceView::new(self.inventory(), self.variant_source_use_sets())
            .serialize(serializer)
    }
}

impl JsonSchema for CatalogProvenance {
    fn schema_name() -> Cow<'static, str> {
        "CatalogProvenance".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        CatalogProvenanceView::json_schema(generator)
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
            | Self::ArchivePathIndex(_)
            | Self::LocationPhysicsTarget(_)
            | Self::MissionTextLength
            | Self::MissionTextAllocation(_)
            | Self::MissionTextPlan
            | Self::LoadoutSourceEvidence(_)
            | Self::SourceOriginMismatch
            | Self::CleanupFailed { .. }
            | Self::Accounting(_)
            | Self::ArchiveChanged(_)
            | Self::Load(_)
            | Self::DataForge(_)
            | Self::LoadTexts(_)
            | Self::Texts { .. }
            | Self::StoreSlugs(_)
            | Self::ShipStorePledges(_)
            | Self::Json(_)
            | Self::Write(_)
            | Self::Cancelled => None,
        }
    }
}

impl CatalogSummary {
    /// Borrows the complete provenance carried by the serialized catalog.
    #[must_use]
    pub const fn provenance(&self) -> &CatalogProvenance {
        &self.provenance
    }

    /// Borrows the immutable source inventory of this actual catalog result.
    #[must_use]
    pub fn source_inventory(&self) -> &SourceInventory {
        self.provenance().inventory()
    }
}
