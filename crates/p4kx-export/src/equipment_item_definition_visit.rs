// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::owner_visit::{EntityScope, EquipmentVisitSummary};
use super::rows::Definitions;
use super::snapshot::ItemDefinitionSnapshot;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

/// Selects the original Item Records in a definition visit.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ItemDefinitionSelection<'records> {
    /// Selects Item Records by the ordinary Catalog Record policy.
    Records(CatalogRecordSelection),
    /// Selects each requested physical Record index once, in increasing index order.
    RecordIndices(&'records [u32]),
}

/// Visits the canonical Item definitions one readable original owner at a time.
///
/// The callback receives the physical Record index and its projected rows.
/// Publish staged changes after terminal success and coverage checks.
///
/// # Errors
/// Returns input, source, cancellation or visitor errors.
pub fn visit_item_definitions<E: From<CatalogError>>(
    archive: &Archive,
    selection: ItemDefinitionSelection<'_>,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(u32, &ItemDefinitionSnapshot<'_>) -> Result<(), E>,
) -> Result<EquipmentVisitSummary, E> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .visit_item_definitions(selection, cancel, visit)
}

/// Visits Item definitions using a decoded Entry from the same Archive.
///
/// # Errors
/// Returns the errors of [`visit_item_definitions`] or an origin mismatch.
pub fn visit_item_definitions_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: ItemDefinitionSelection<'_>,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(u32, &ItemDefinitionSnapshot<'_>) -> Result<(), E>,
) -> Result<EquipmentVisitSummary, E> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .visit_item_definitions(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits selected Item definitions with this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`visit_item_definitions`].
    pub fn visit_item_definitions<E: From<CatalogError>>(
        &self,
        selection: ItemDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(u32, &ItemDefinitionSnapshot<'_>) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        self.parsed(cancel)?
            .visit_item_definitions(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits selected Item definitions in physical order using this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`visit_item_definitions`].
    pub fn visit_item_definitions<E: From<CatalogError>>(
        &self,
        selection: ItemDefinitionSelection<'_>,
        cancel: Option<&CancelToken>,
        mut visit: impl FnMut(u32, &ItemDefinitionSnapshot<'_>) -> Result<(), E>,
    ) -> Result<EquipmentVisitSummary, E> {
        let (records, indices) = match selection {
            ItemDefinitionSelection::Records(records) => (records, None),
            ItemDefinitionSelection::RecordIndices(indices) => {
                (CatalogRecordSelection::All, Some(indices))
            }
        };
        self.visit_equipment_owners::<Definitions, E>(
            records,
            EntityScope::Items(indices),
            cancel,
            |index, snapshot| visit(index, &snapshot),
        )
    }
}
