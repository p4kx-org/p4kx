// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, Language, LocalizationNote, SourceInventory, SourcedValue};

use crate::blueprint_definition::{BlueprintDefinition, PhysicalBlueprintRef};
use crate::blueprint_presentation::BlueprintPresentation;
use crate::blueprint_tier_contract::BlueprintTier;
use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;

/// Coverage of the original families needed for selected Blueprint presentation.
#[derive(Debug, Default)]
#[must_use]
pub struct BlueprintDependencyCoverage {
    pub(super) globals: CatalogRecordCoverage,
    pub(super) dismantle: CatalogRecordCoverage,
    pub(super) categories: CatalogRecordCoverage,
}

impl BlueprintDependencyCoverage {
    /// Borrows the required crafting-global default and blacklist survey coverage.
    #[must_use]
    pub const fn globals(&self) -> &CatalogRecordCoverage {
        &self.globals
    }

    /// Borrows coverage of dismantle candidates actually demanded by selected presentation.
    #[must_use]
    pub const fn dismantle(&self) -> &CatalogRecordCoverage {
        &self.dismantle
    }

    /// Borrows coverage of the original referenced category candidates.
    #[must_use]
    pub const fn categories(&self) -> &CatalogRecordCoverage {
        &self.categories
    }
}

/// Unrecognized authored modifier-range types associated with one physical Blueprint.
#[derive(Debug)]
#[must_use]
pub struct BlueprintUnsupportedRanges<'source> {
    pub(super) blueprint: PhysicalBlueprintRef,
    pub(super) source_types: Vec<&'source str>,
}

impl<'source> BlueprintUnsupportedRanges<'source> {
    /// Borrows the exact physical owner of these source range types.
    #[must_use]
    pub const fn blueprint(&self) -> &PhysicalBlueprintRef {
        &self.blueprint
    }

    /// Borrows unrecognized original range-type occurrences without interpreting a formula.
    #[must_use]
    pub fn source_types(&self) -> &[&'source str] {
        &self.source_types
    }
}

/// Selected physical Blueprint definitions with their tiers, presentation and source context.
#[derive(Debug)]
#[must_use]
pub struct BlueprintSnapshot<'source> {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) definitions: Vec<BlueprintDefinition<'source>>,
    pub(super) tiers: Vec<BlueprintTier>,
    pub(super) presentations: Vec<BlueprintPresentation<SourcedValue<CatalogGuid>>>,
    pub(super) public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    pub(super) game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    pub(super) requested_languages: Vec<Language>,
    pub(super) available_languages: Vec<Language>,
    pub(super) localization_notes: Vec<LocalizationNote>,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) dependency_coverage: BlueprintDependencyCoverage,
    pub(super) unsupported_ranges: Vec<BlueprintUnsupportedRanges<'source>>,
}

impl<'source> BlueprintSnapshot<'source> {
    /// Borrows the originating installed Build when identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns source-family declaration availability independently of the selected row count.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows full definitions in canonical GUID-text and physical Record order.
    #[must_use]
    pub fn definitions(&self) -> &[BlueprintDefinition<'source>] {
        &self.definitions
    }

    /// Borrows all selected-owner tiers at their original positions.
    #[must_use]
    pub fn tiers(&self) -> &[BlueprintTier] {
        &self.tiers
    }

    /// Borrows canonical per-owner names, references and default/dismantle choices.
    #[must_use]
    pub fn presentations(&self) -> &[BlueprintPresentation<SourcedValue<CatalogGuid>>] {
        &self.presentations
    }

    /// Borrows referenced public-name rows in this result's text context.
    #[must_use]
    pub fn public_names(&self) -> &[CanonicalText<'source, PublicNameRef>] {
        &self.public_names
    }

    /// Borrows referenced protected-text rows in this result's text context.
    #[must_use]
    pub fn game_texts(&self) -> &[CanonicalText<'source, GameTextRef>] {
        &self.game_texts
    }

    /// Resolves a protected-text identity within this result's own registry.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|text| text.id.value() == Some(reference))
    }

    /// Resolves a public-name identity within this result's own registry.
    #[must_use]
    pub fn public_name(
        &self,
        reference: &PublicNameRef,
    ) -> Option<&CanonicalText<'source, PublicNameRef>> {
        self.public_names.iter().find(|text| {
            text.id
                .value()
                .is_some_and(|id| id.as_str() == reference.as_str())
        })
    }

    /// Borrows the requested canonical locale policy, including unavailable languages.
    #[must_use]
    pub fn requested_languages(&self) -> &[Language] {
        &self.requested_languages
    }

    /// Borrows the requested localization payloads actually present and read.
    #[must_use]
    pub fn available_languages(&self) -> &[Language] {
        &self.available_languages
    }

    /// Borrows decoding and parsing notes from requested localization inputs.
    #[must_use]
    pub fn localization_notes(&self) -> &[LocalizationNote] {
        &self.localization_notes
    }

    /// Borrows the original physical sources and per-datum uses of this result.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows selected owner-header, body and field coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }

    /// Borrows demanded dependency-family coverage independently of owner coverage.
    #[must_use]
    pub const fn dependency_coverage(&self) -> &BlueprintDependencyCoverage {
        &self.dependency_coverage
    }

    /// Borrows unrecognized range types with their original physical owners.
    #[must_use]
    pub fn unsupported_ranges(&self) -> &[BlueprintUnsupportedRanges<'source>] {
        &self.unsupported_ranges
    }
}
