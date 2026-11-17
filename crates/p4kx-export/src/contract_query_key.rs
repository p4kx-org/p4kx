// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Guid};

use super::{CatalogLanguageSelection, ContractSnapshot, plan, with_parsed};
use crate::CatalogError;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries};

/// The exact logical parent or sub-contract identity supplied by an SDK caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct ContractKey {
    contract_guid: Guid,
    sub_contract_guid: Option<Guid>,
}

impl ContractKey {
    /// Selects the parent row when `sub_contract_guid` is absent, otherwise one sub-contract row.
    pub const fn new(contract_guid: Guid, sub_contract_guid: Option<Guid>) -> Self {
        Self {
            contract_guid,
            sub_contract_guid,
        }
    }

    /// The original parent contract identity.
    #[must_use]
    pub const fn contract_guid(self) -> Guid {
        self.contract_guid
    }

    /// The original sub-contract identity, absent when selecting the parent row.
    #[must_use]
    pub const fn sub_contract_guid(self) -> Option<Guid> {
        self.sub_contract_guid
    }

    pub(super) const fn identity(self) -> super::survey::LogicalKey {
        (self.contract_guid, self.sub_contract_guid)
    }
}

/// Supplies the global canonical row for one logical contract identity and its dependencies.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence, Archive change or cancellation errors.
pub fn with_contract<T>(
    archive: &Archive,
    key: ContractKey,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?.with_contract(key, cancel, inspect)
}

/// Supplies one logical contract from an already decoded Entry of the same Archive.
///
/// # Errors
/// Returns the errors of `with_contract`, including source-origin mismatch.
pub fn with_contract_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    key: ContractKey,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_contract(key, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies one logical contract using this session's retained original DataForge.
    ///
    /// # Errors
    /// Returns the errors of `with_contract`.
    pub fn with_contract<T>(
        &self,
        key: ContractKey,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?.with_contract(key, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies one logical contract using this view's retained DataForge indexes.
    ///
    /// # Errors
    /// Returns the errors of `with_contract`.
    pub fn with_contract<T>(
        &self,
        key: ContractKey,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(
                archive,
                entry,
                forge,
                plan::Lookup::Logical(key),
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}
