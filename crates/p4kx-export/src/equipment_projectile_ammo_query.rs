// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{read, weapon_rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use super::projectile_ammo_read::ProjectileAmmoOwner;
pub use crate::catalog_v3::catalog_weapon_projectile_ammo::{
    ProjectileAmmoChoice, ProjectileAmmoRole, WeaponProjectileAmmoSelection,
};

/// Original projectile routes and reached Ammo definitions with their source inventory.
pub type ProjectileAmmoSnapshot<'source> =
    super::EquipmentSnapshot<'source, ProjectileAmmoOwner<'source>>;

/// Supplies canonical projectile routes and their reached ammunition for one physical Item.
/// Publish consumer changes after outer success and the required coverage checks.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_projectile_ammo<T>(
    archive: &Archive,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ProjectileAmmoSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<weapon_rows::ProjectileAmmo, T>(
        archive,
        None,
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same canonical rows from a decoded Entry of the same Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_projectile_ammo_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ProjectileAmmoSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<weapon_rows::ProjectileAmmo, T>(
        archive,
        Some(entry),
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies one physical Item's projectile routes and reached Ammo through this session.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_projectile_ammo<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ProjectileAmmoSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_projectile_ammo(record_index, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies one physical Item's projectile routes and reached Ammo through parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_projectile_ammo<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ProjectileAmmoSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<weapon_rows::ProjectileAmmo, T>(
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
