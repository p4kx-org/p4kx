// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::snapshot::EquipmentSnapshot;
use super::{read, weapon_rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::catalog_v3::catalog_ammo_bindings::{
    AmmoBinding, ContainerAmmoSlots, SecondaryMagazine, WeaponMagazineSlots,
};
pub use crate::catalog_v3::catalog_ammo_containers::AmmoContainer;
pub use crate::catalog_v3::catalog_item_components::WeaponSource;
pub use crate::catalog_v3::catalog_weapon_actions::{
    ProjectileCombatInputs, WeaponAction, WeaponActions, WeaponModeKind,
};

/// Authored weapon and ammo component rows with their original source inventory.
pub type WeaponSourceSnapshot<'source> = EquipmentSnapshot<'source, WeaponSource<'source>>;

/// Supplies original weapon actions and ammunition bindings on one physical Item.
/// Use the returned coverage before treating empty rows as absence.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_weapon_sources<T>(
    archive: &Archive,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<weapon_rows::WeaponSources, T>(
        archive,
        None,
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same original rows from an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_sources_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponSourceSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<weapon_rows::WeaponSources, T>(
        archive,
        Some(entry),
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies the selected Item's authored weapon rows through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_sources<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_sources(record_index, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the selected Item's authored weapon rows through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_sources<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponSourceSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<weapon_rows::WeaponSources, T>(
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
