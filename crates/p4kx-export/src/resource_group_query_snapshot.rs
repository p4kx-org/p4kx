// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, Language, LocalizationNote, SourceInventory};

use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::catalog_v3::resource_group_catalog::{
    ResourceGroupDefinition, ResourceGroupMembers, ResourceGroupParent,
};

/// Selected direct physical resource group definitions and original ordered declarations.
#[derive(Debug)]
#[must_use]
pub struct ResourceGroupSnapshot<'source> {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) definitions: Vec<ResourceGroupDefinition<'source>>,
    pub(super) members: Vec<ResourceGroupMembers>,
    pub(super) public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    pub(super) game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    pub(super) requested_languages: Vec<Language>,
    pub(super) available_languages: Vec<Language>,
    pub(super) localization_notes: Vec<LocalizationNote>,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
}

/// Selected parent choices with explicit canonical group-family dependency coverage.
#[derive(Debug)]
#[must_use]
pub struct ResourceGroupParentSnapshot {
    pub(super) build: Option<Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) rows: Vec<ResourceGroupParent>,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) dependency_coverage: CatalogRecordCoverage,
}

impl<'source> ResourceGroupSnapshot<'source> {
    /// Borrows the installed Build when identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns source-family declaration availability independently of row count.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows canonical direct definitions in GUID-text and physical order.
    #[must_use]
    pub fn definitions(&self) -> &[ResourceGroupDefinition<'source>] {
        &self.definitions
    }

    /// Borrows every original membership slot in matching physical owner order.
    #[must_use]
    pub fn members(&self) -> &[ResourceGroupMembers] {
        &self.members
    }

    /// Borrows public-name rows in this result context.
    #[must_use]
    pub fn public_names(&self) -> &[CanonicalText<'source, PublicNameRef>] {
        &self.public_names
    }

    /// Borrows protected-text rows in this result context.
    #[must_use]
    pub fn game_texts(&self) -> &[CanonicalText<'source, GameTextRef>] {
        &self.game_texts
    }

    /// Borrows the requested localization policy.
    #[must_use]
    pub fn requested_languages(&self) -> &[Language] {
        &self.requested_languages
    }

    /// Borrows requested locale payloads actually read.
    #[must_use]
    pub fn available_languages(&self) -> &[Language] {
        &self.available_languages
    }

    /// Borrows localization decoding and parsing notes.
    #[must_use]
    pub fn localization_notes(&self) -> &[LocalizationNote] {
        &self.localization_notes
    }

    /// Borrows physical sources and exact per-datum uses.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows selected owner header, body and field coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }

    /// Resolves a public name only inside this result's text context.
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

    /// Resolves protected text only inside this result's text context.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|row| row.id.value() == Some(reference))
    }
}

impl ResourceGroupParentSnapshot {
    /// Borrows the installed Build when identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns source-family declaration availability independently of selected parents.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows canonical parent choices in GUID-text and physical child order.
    #[must_use]
    pub fn rows(&self) -> &[ResourceGroupParent] {
        &self.rows
    }

    /// Borrows actual parent-selection sources and separate failed attempts.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows selected child-owner coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }

    /// Borrows the complete demanded group-family survey coverage.
    #[must_use]
    pub const fn dependency_coverage(&self) -> &CatalogRecordCoverage {
        &self.dependency_coverage
    }
}
