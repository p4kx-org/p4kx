// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::read;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::location_vocabulary;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::location_vocabulary::{ContractScopeGlobalParams, ContractScopeItem};

/// Selected original contract scopes, public names and original source evidence.
pub type ContractScopeSnapshot<'source> =
    super::EquipmentSnapshot<'source, ContractScopeGlobalParams<'source>>;

/// Reads selected physical contract-scope Records and their original public names.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_contract_scopes<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractScopeSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<ContractScopes, T>(archive, None, selection, languages, cancel, inspect)
}

/// Reads selected scopes from this Archive's already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_contract_scopes_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractScopeSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<ContractScopes, T>(
        archive,
        Some(entry),
        selection,
        languages,
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Reads original contract scopes using this session and its locale policy.
    ///
    /// # Errors
    /// Returns source-evidence, Archive-change or cancellation errors.
    pub fn with_contract_scopes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractScopeSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_scopes(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads original contract scopes from the session's parsed DataForge.
    ///
    /// # Errors
    /// Returns source-evidence, Archive-change or cancellation errors.
    pub fn with_contract_scopes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractScopeSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<ContractScopes, T>(
                archive,
                entry,
                forge,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

struct ContractScopes;

impl Projector for ContractScopes {
    type Row<'source> = ContractScopeGlobalParams<'source>;
    const FAMILY: Family = Family::ContractScopes;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read =
            location_vocabulary::read_contract_scope_selected(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row]))
    }
}
