// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original actor recoil configuration and modifier Records.

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::procedural_definitions;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::weapon_procedural_definitions::{
    self, RECOIL_CONFIG_SOURCE_TYPE, RECOIL_MODIFIERS_SOURCE_TYPE,
};

pub use crate::catalog_v3::weapon_procedural_definitions::{
    ActorProceduralRecoilConfig, ActorProceduralRecoilModifier, ActorProceduralRecoilSetup,
    ActorStateFilter,
};

/// Selected original actor recoil configurations and their sources.
pub type ActorProceduralRecoilConfigSnapshot<'source> =
    EquipmentSnapshot<'source, ActorProceduralRecoilConfig<'source>>;
/// Selected original actor recoil modifiers and their sources.
pub type ActorProceduralRecoilModifierSnapshot<'source> =
    EquipmentSnapshot<'source, ActorProceduralRecoilModifier<'source>>;

/// Supplies original actor recoil configurations for the selected physical Records.
///
/// # Errors
/// Returns input, source-accounting, Archive-change or cancellation errors.
pub fn with_actor_procedural_recoil_configs<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ActorProceduralRecoilConfigSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_actor_procedural_recoil_configs(selection, cancel, inspect)
}

/// Supplies selected actor recoil configurations using the original decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_actor_procedural_recoil_configs_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ActorProceduralRecoilConfigSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_actor_procedural_recoil_configs(selection, cancel, inspect)
}

/// Supplies original actor recoil modifiers for the selected physical Records.
///
/// # Errors
/// Returns input, source-accounting, Archive-change or cancellation errors.
pub fn with_actor_procedural_recoil_modifiers<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ActorProceduralRecoilModifierSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_actor_procedural_recoil_modifiers(selection, cancel, inspect)
}

/// Supplies selected actor recoil modifiers using the original decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_actor_procedural_recoil_modifiers_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ActorProceduralRecoilModifierSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_actor_procedural_recoil_modifiers(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected actor recoil configurations through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_actor_procedural_recoil_configs<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ActorProceduralRecoilConfigSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_actor_procedural_recoil_configs(selection, cancel, inspect)
    }

    /// Supplies selected actor recoil modifiers through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_actor_procedural_recoil_modifiers<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ActorProceduralRecoilModifierSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_actor_procedural_recoil_modifiers(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected actor recoil configurations through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_actor_procedural_recoil_configs<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ActorProceduralRecoilConfigSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            procedural_definitions::with_parsed(
                archive,
                entry,
                forge,
                selection,
                (
                    RECOIL_CONFIG_SOURCE_TYPE,
                    weapon_procedural_definitions::read_recoil_config_selected,
                ),
                cancel,
                inspect,
            )
        })
    }

    /// Supplies selected actor recoil modifiers through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_actor_procedural_recoil_modifiers<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ActorProceduralRecoilModifierSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            procedural_definitions::with_parsed(
                archive,
                entry,
                forge,
                selection,
                (
                    RECOIL_MODIFIERS_SOURCE_TYPE,
                    weapon_procedural_definitions::read_procedural_target_selected,
                ),
                cancel,
                inspect,
            )
        })
    }
}
