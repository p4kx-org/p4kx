// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{read, weapon_rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use super::weapon_combat_read::WeaponCombatOwner;
pub use crate::catalog_v3::weapon_combat_base::{
    AlphaUnit, CombatModel, CombatUnits, DamageChannels, DistanceUnit, DpsUnit, FireRateUnit,
    NominalBullet, NominalBulletDamage, WeaponCombatBase,
};

/// Authored fire modes, nominal combat metrics and their original dependencies.
pub type WeaponCombatSnapshot<'source> =
    super::EquipmentSnapshot<'source, WeaponCombatOwner<'source>>;

/// Supplies the canonical combat rows and their dependencies for one physical Item.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_weapon_combat_bases<T>(
    archive: &Archive,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponCombatSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<weapon_rows::WeaponCombatBases, T>(
        archive,
        None,
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same combat rows using a decoded DataForge Entry of this Archive.
///
/// # Errors
/// Returns the query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_combat_bases_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponCombatSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<weapon_rows::WeaponCombatBases, T>(
        archive,
        Some(entry),
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies one physical Item's canonical combat rows through this Archive session.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_combat_bases<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponCombatSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_combat_bases(record_index, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies one physical Item's canonical combat rows through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_combat_bases<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponCombatSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<weapon_rows::WeaponCombatBases, T>(
                archive,
                entry,
                forge,
                CatalogRecordSelection::RecordIndex(record_index),
                &[],
                cancel,
                inspect,
            )
        })
    }
}
