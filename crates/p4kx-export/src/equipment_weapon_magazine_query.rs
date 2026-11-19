// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::catalog_v3::catalog_item_components::resolution::{
    Outcome as WeaponMagazineOutcome, WeaponMagazineResolution,
};

/// Original primary magazine resolutions and their original source inventory.
pub type WeaponMagazineSnapshot<'source> =
    super::EquipmentSnapshot<'source, WeaponMagazineResolution>;

/// Supplies the canonical primary magazine route for one physical Item.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_weapon_magazine_resolutions<T>(
    archive: &Archive,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponMagazineSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::MagazineResolutions, T>(
        archive,
        None,
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same primary route using a decoded DataForge Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_magazine_resolutions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponMagazineSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::MagazineResolutions, T>(
        archive,
        Some(entry),
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies one physical Item's primary route through this Archive session.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_magazine_resolutions<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponMagazineSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_magazine_resolutions(record_index, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies one physical Item's primary route through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_magazine_resolutions<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponMagazineSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::MagazineResolutions, T>(
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
