// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to the original item recovery policy and its three catalog projections.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, Language, LocalizationNote, SourceInventory,
};

use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::CatalogError;
use crate::catalog_text_contract::{CanonicalText, GameTextRef};

pub use crate::contract_recovery::{
    ItemRecoveryEconomy, ItemRecoveryOverrides, RecoveryCooldown, RecoveryDelivery,
    RecoveryEconomy, RecoveryEscalating, RecoveryLegacy, RecoveryOverride, RecoveryRepair,
};
pub use crate::recovery::policy_contract::{
    ConditionList, ItemRecoveryPolicy, RecoveryCondition, RecoveryConditionPayload,
    RecoveryConditionTags, RecoveryNotifications,
};

#[path = "item_recovery_query_read.rs"]
mod read;

/// The three projections of one original item recovery policy with their source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ItemRecoverySnapshot<'source> {
    /// The installed Build when available.
    pub build: Option<Build>,
    /// Availability of the item recovery source type.
    pub availability: DatasetAvailability,
    /// The economy projection when a selected policy is available.
    pub economy: Option<ItemRecoveryEconomy<'source>>,
    /// The override projection of the same physical policy.
    pub overrides: Option<ItemRecoveryOverrides>,
    /// The policy and protected notification references of the same physical owner.
    pub policy: Option<ItemRecoveryPolicy<'source>>,
    /// The protected text rows referenced by the selected policy.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested languages including unavailable locales.
    pub requested_languages: Vec<Language>,
    /// Requested languages whose entries were read.
    pub available_languages: Vec<Language>,
    /// Notes from the localization inputs consulted by this query.
    pub localization_notes: Vec<LocalizationNote>,
    /// The physical sources and uses for this result.
    pub source_inventory: SourceInventory,
    /// Read and projection coverage of the selected Record scope.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> ItemRecoverySnapshot<'source> {
    /// Resolves a protected notification reference against the texts in this snapshot.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|text| text.id().value() == Some(reference))
    }
}

/// Supplies the first readable admitted policy in original path and physical Record order.
/// The selection narrows the candidates for all three projections.
///
/// # Errors
/// Returns input, source-accounting or cancellation errors.
pub fn with_item_recovery<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemRecoverySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_item_recovery(selection, cancel, inspect)
}

/// Supplies item recovery from an already decoded DataForge Entry of the same Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_item_recovery_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemRecoverySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_item_recovery(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies the selected policy projections using this query session.
    ///
    /// # Errors
    /// Returns input, source-accounting or cancellation errors.
    pub fn with_item_recovery<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemRecoverySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_item_recovery(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the selected policy projections using this parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source-accounting or cancellation errors.
    pub fn with_item_recovery<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemRecoverySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::Query {
                archive,
                entry,
                forge,
                selection,
                requested_languages: self.languages(),
                cancel,
            }
            .run(inspect)
        })
    }
}
