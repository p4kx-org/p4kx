// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, Language, LocalizationNote, SourceInventory};

use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::mining_locations::{MineableComposition, MineableElement};
use crate::resource_type_contract::ResourceDefinition;

/// Selected canonical definitions with their own text and original source context.
#[derive(Debug)]
#[must_use]
pub struct MiningDefinitionSnapshot<'source, Row> {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) rows: Vec<Row>,
    pub(super) public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    pub(super) game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    pub(super) requested_languages: Vec<Language>,
    pub(super) available_languages: Vec<Language>,
    pub(super) localization_notes: Vec<LocalizationNote>,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) dependency_coverage: CatalogRecordCoverage,
}

/// Selected mineable elements without world or provider discovery.
pub type MineableElementSnapshot<'source> = MiningDefinitionSnapshot<'source, MineableElement>;

/// Selected mineable compositions without world or provider discovery.
pub type MineableCompositionSnapshot<'source> =
    MiningDefinitionSnapshot<'source, MineableComposition>;

/// Selected complete `ResourceType` definitions and demanded membership evidence.
pub type ResourceTypeSnapshot<'source> =
    MiningDefinitionSnapshot<'source, ResourceDefinition<'source>>;

impl<'source, Row> MiningDefinitionSnapshot<'source, Row> {
    /// Borrows the originating installed Build when one was identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns availability of the canonical source family.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows canonical GUID-ordered rows with physical order for equal GUIDs.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Borrows referenced public-name rows in this snapshot's text context.
    #[must_use]
    pub fn public_names(&self) -> &[CanonicalText<'source, PublicNameRef>] {
        &self.public_names
    }

    /// Borrows referenced protected-text rows in this snapshot's text context.
    #[must_use]
    pub fn game_texts(&self) -> &[CanonicalText<'source, GameTextRef>] {
        &self.game_texts
    }

    /// Resolves a public-name identity only within this result's text arena.
    #[must_use]
    pub fn public_name(
        &self,
        reference: &PublicNameRef,
    ) -> Option<&CanonicalText<'source, PublicNameRef>> {
        self.public_names.iter().find(|row| {
            row.id
                .value()
                .is_some_and(|id| id.as_str() == reference.as_str())
        })
    }

    /// Resolves a protected-text identity only within this result's text arena.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|row| row.id.value() == Some(reference))
    }

    /// Borrows the requested language policy, including unavailable payloads.
    #[must_use]
    pub fn requested_languages(&self) -> &[Language] {
        &self.requested_languages
    }

    /// Borrows the requested language payloads actually available to this query.
    #[must_use]
    pub fn available_languages(&self) -> &[Language] {
        &self.available_languages
    }

    /// Borrows decoding and parsing notes from the requested localization inputs.
    #[must_use]
    pub fn localization_notes(&self) -> &[LocalizationNote] {
        &self.localization_notes
    }

    /// Borrows the exact source/use inventory of the returned cells.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows selected owner-header, body and field coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }

    /// Borrows demanded ResourceGroup-family coverage, empty for element/composition reads.
    #[must_use]
    pub const fn dependency_coverage(&self) -> &CatalogRecordCoverage {
        &self.dependency_coverage
    }
}
