// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::super::owner_visit::EntityScope;
use super::{InteractionSnapshot, Interactions};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_interactions::SOURCE_TYPES;

/// Outcome of visiting the selected original interaction components.
pub type InteractionVisitSummary = super::super::EquipmentVisitSummary;

/// Visits readable interaction candidates in physical Record order with owner-local sources.
/// Stage consumer changes until the visit succeeds and its coverage meets the caller's requirements.
///
/// # Errors
///
/// Returns input, Archive-change, cancellation or visitor errors.
pub fn visit_interactions<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
) -> Result<InteractionVisitSummary, E> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .visit_interactions(selection, cancel, visit)
}

/// Visits interactions from the same Archive's previously decoded DataForge Entry.
///
/// # Errors
///
/// Returns query or visitor errors, including a source-origin mismatch for a foreign Entry.
pub fn visit_interactions_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
) -> Result<InteractionVisitSummary, E> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .visit_interactions(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits selected physical interaction owners under this session's locale policy.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_interactions<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<InteractionVisitSummary, E> {
        self.parsed(cancel)?
            .visit_interactions(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits selected physical interaction owners using the parsed original DataForge.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_interactions<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        mut visit: impl for<'source> FnMut(u32, &InteractionSnapshot<'source>) -> Result<(), E>,
    ) -> Result<InteractionVisitSummary, E> {
        self.visit_equipment_owners::<Interactions, E>(
            selection,
            EntityScope::Components(SOURCE_TYPES),
            cancel,
            |index, snapshot| visit(index, &snapshot),
        )
    }
}
