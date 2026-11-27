// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original weapon procedural animation and clip Records.

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::procedural_definitions;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::weapon_procedural_definitions::{
    self, ANIMATION_SOURCE_TYPE, CLIP_SOURCE_TYPE,
};

pub use crate::catalog_v3::weapon_procedural_definitions::{
    WeaponProceduralAnimation, WeaponProceduralClip, WeaponProceduralClipSetup,
};

/// Selected original weapon procedural animations and their sources.
pub type WeaponProceduralAnimationSnapshot<'source> =
    EquipmentSnapshot<'source, WeaponProceduralAnimation<'source>>;

/// Selected original weapon procedural clips and their sources.
pub type WeaponProceduralClipSnapshot<'source> =
    EquipmentSnapshot<'source, WeaponProceduralClip<'source>>;

/// Supplies procedural animations for the selected physical Records.
///
/// # Errors
/// Returns input, source-accounting, Archive-change or cancellation errors.
pub fn with_weapon_procedural_animations<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponProceduralAnimationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_weapon_procedural_animations(selection, cancel, inspect)
}

/// Supplies selected procedural animations from the original decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_procedural_animations_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponProceduralAnimationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_weapon_procedural_animations(selection, cancel, inspect)
}

/// Supplies procedural clips for the selected physical Records.
///
/// # Errors
/// Returns input, source-accounting, Archive-change or cancellation errors.
pub fn with_weapon_procedural_clips<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponProceduralClipSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_weapon_procedural_clips(selection, cancel, inspect)
}

/// Supplies selected procedural clips from the original decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_procedural_clips_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponProceduralClipSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_weapon_procedural_clips(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected procedural animations through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_procedural_animations<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponProceduralAnimationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_procedural_animations(selection, cancel, inspect)
    }

    /// Supplies selected procedural clips through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_procedural_clips<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponProceduralClipSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_procedural_clips(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected procedural animations through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_procedural_animations<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponProceduralAnimationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            procedural_definitions::with_parsed(
                archive,
                entry,
                forge,
                selection,
                (
                    ANIMATION_SOURCE_TYPE,
                    weapon_procedural_definitions::read_animation_selected,
                ),
                cancel,
                inspect,
            )
        })
    }

    /// Supplies selected procedural clips through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_procedural_clips<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponProceduralClipSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            procedural_definitions::with_parsed(
                archive,
                entry,
                forge,
                selection,
                (
                    CLIP_SOURCE_TYPE,
                    weapon_procedural_definitions::read_procedural_target_selected,
                ),
                cancel,
                inspect,
            )
        })
    }
}
