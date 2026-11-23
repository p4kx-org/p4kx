// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, Language, LocalizationNote, SourceInventory};

use super::{InventoryOwnerBounds, Item, ItemAttachmentComponent, ItemDefinition, ItemSource};
use crate::CatalogProvenanceView;
use crate::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::variant_use_sets::VariantSourceUseSets;

/// Selected canonical original equipment rows with their exact source and text context.
#[derive(Debug)]
#[must_use]
pub struct EquipmentSnapshot<'source, Row, Dependencies = ()> {
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
    pub(super) dependency_coverage: Dependencies,
}

/// Selected original Item presentation, physics and purchase definitions.
pub type ItemDefinitionSnapshot<'source> = EquipmentSnapshot<'source, ItemDefinition<'source>>;

/// Selected canonical physical Item rows with variant decisions.
pub type ItemSnapshot<'source> = EquipmentSnapshot<'source, Item<'source>, VariantSourceUseSets>;

impl<'source> EquipmentSnapshot<'source, Item<'source>> {
    pub(super) fn with_variant_sets(self, sets: VariantSourceUseSets) -> ItemSnapshot<'source> {
        ItemSnapshot {
            build: self.build,
            availability: self.availability,
            rows: self.rows,
            public_names: self.public_names,
            game_texts: self.game_texts,
            requested_languages: self.requested_languages,
            available_languages: self.available_languages,
            localization_notes: self.localization_notes,
            source_inventory: self.source_inventory,
            coverage: self.coverage,
            dependency_coverage: sets,
        }
    }
}

impl ItemSnapshot<'_> {
    /// Borrows the complete canonical provenance for this Item query.
    #[must_use]
    pub const fn provenance(&self) -> CatalogProvenanceView<'_> {
        CatalogProvenanceView::new(self.source_inventory(), self.variant_source_use_sets())
    }

    /// Borrows the use sets referenced by the emitted Item variant decisions.
    #[must_use]
    pub const fn variant_source_use_sets(&self) -> &VariantSourceUseSets {
        &self.dependency_coverage
    }
}

/// Selected original Item headers, component choices, class and playability.
pub type ItemSourceSnapshot<'source> = EquipmentSnapshot<'source, ItemSource<'source>>;

/// Selected original physical attachment-constraint components.
pub type AttachmentConstraintSnapshot<'source> =
    EquipmentSnapshot<'source, ItemAttachmentComponent<'source>>;

/// Selected original storage occupancy, vehicle bounds and object metadata components.
pub type ItemBoundsSnapshot<'source> = EquipmentSnapshot<'source, InventoryOwnerBounds<'source>>;

impl<'source, Row, Dependencies> EquipmentSnapshot<'source, Row, Dependencies> {
    /// Borrows the installed Build when one was identified.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns availability of the declared canonical family.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.availability
    }

    /// Borrows canonical rows in their original family ordering.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Borrows public-name definitions reached by this request.
    #[must_use]
    pub fn public_names(&self) -> &[CanonicalText<'source, PublicNameRef>] {
        &self.public_names
    }

    /// Borrows protected-text definitions reached by this request.
    #[must_use]
    pub fn game_texts(&self) -> &[CanonicalText<'source, GameTextRef>] {
        &self.game_texts
    }

    /// Borrows the requested presentation languages retained by this family.
    #[must_use]
    pub fn requested_languages(&self) -> &[Language] {
        &self.requested_languages
    }

    /// Borrows requested languages whose payloads were successfully read.
    #[must_use]
    pub fn available_languages(&self) -> &[Language] {
        &self.available_languages
    }

    /// Borrows decoding and parser observations from the consulted locale inputs.
    #[must_use]
    pub fn localization_notes(&self) -> &[LocalizationNote] {
        &self.localization_notes
    }

    /// Borrows the source inventory for these returned cells.
    #[must_use]
    pub const fn source_inventory(&self) -> &SourceInventory {
        &self.source_inventory
    }

    /// Borrows original header, body and field projection coverage.
    #[must_use]
    pub const fn coverage(&self) -> &CatalogRecordCoverage {
        &self.coverage
    }

    /// Resolves a public-name identity within this result's own text context.
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

    /// Resolves a protected-text identity within this result's own text context.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts.iter().find(|row| {
            row.id
                .value()
                .is_some_and(|id| id.as_str() == reference.as_str())
        })
    }
}
