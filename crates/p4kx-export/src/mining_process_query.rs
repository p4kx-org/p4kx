// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Direct canonical mining globals and refining definitions without world discovery.

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use super::snapshot::MiningDefinitionSnapshot;
pub use crate::catalog_v3::mining_process_catalog::{
    MiningExplosion, MiningGlobalConfiguration, MiningHitConsistency, MiningInstability,
    RefiningProcessDefinition,
};

/// Selected physical refining definitions and their original source/text context.
pub type RefiningProcessSnapshot<'source> =
    MiningDefinitionSnapshot<'source, RefiningProcessDefinition<'source>>;
/// Selected physical mining globals with complete original source values.
pub type MiningGlobalSnapshot<'source> =
    MiningDefinitionSnapshot<'source, MiningGlobalConfiguration<'source>>;

/// Supplies complete selected refining processes without a world or `ResourceType` survey.
/// Publish owned changes after outer success and coverage examination.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_refining_processes<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(RefiningProcessSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Refining, T>(archive, None, selection, languages, cancel, inspect)
}

/// Supplies selected refining definitions from an Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence or cancellation errors.
pub fn with_refining_processes_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(RefiningProcessSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Refining, T>(
        archive,
        Some(entry),
        selection,
        languages,
        cancel,
        inspect,
    )
}

/// Supplies complete selected mining globals without localization or world discovery.
/// Publish owned changes after outer success and coverage examination.
///
/// # Errors
/// Returns input, source-evidence or cancellation errors.
pub fn with_mining_global_params<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MiningGlobalSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Globals, T>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies complete mining globals from an Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence or cancellation errors.
pub fn with_mining_global_params_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MiningGlobalSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::Globals, T>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies selected refining processes using this session's language policy.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_refining_processes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(RefiningProcessSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_refining_processes(selection, cancel, inspect)
    }

    /// Supplies selected original mining globals without opening locale payloads.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_mining_global_params<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MiningGlobalSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mining_global_params(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected refining processes while reusing parsed DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_refining_processes<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(RefiningProcessSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::Refining, T>(
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

    /// Supplies selected mining globals while reusing parsed DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence or cancellation errors.
    pub fn with_mining_global_params<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MiningGlobalSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::Globals, T>(
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
