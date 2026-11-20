// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Filter};

use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::catalog_v3::catalog_item_components::resolution::{
    Outcome as WeaponMagazineOutcome, WeaponMagazineResolution,
};

pub use super::weapon_magazine_dependency_read::{
    LinkedMagazineSources, WeaponMagazineDependencyOwner,
};

/// Canonical primary magazine dependency rows and their original source inventory.
pub type WeaponMagazineDependencySnapshot<'source> =
    super::EquipmentSnapshot<'source, WeaponMagazineDependencyOwner<'source>>;

/// Supplies one physical Item's primary route and any out-of-selection container and binding.
///
/// The optional filter applies the Catalog Item admission policy to the weapon and magazine.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_weapon_magazine_dependencies<T>(
    archive: &Archive,
    record_index: u32,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponMagazineDependencySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_weapon_magazine_dependencies(record_index, filter, cancel, inspect)
}

/// Supplies the same magazine dependency rows using a decoded DataForge Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_magazine_dependencies_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponMagazineDependencySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_weapon_magazine_dependencies(record_index, filter, cancel, inspect)
}

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
    /// Supplies one physical Item's canonical primary route and out-of-selection magazine rows.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_magazine_dependencies<T>(
        &self,
        record_index: u32,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponMagazineDependencySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?.with_weapon_magazine_dependencies(
            record_index,
            filter,
            cancel,
            inspect,
        )
    }

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
    /// Supplies the canonical primary route and magazine dependency rows of a selected Item.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive-change or cancellation errors.
    pub fn with_weapon_magazine_dependencies<T>(
        &self,
        record_index: u32,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponMagazineDependencySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::Query {
                archive,
                entry,
                forge,
                selection: CatalogRecordSelection::RecordIndex(record_index),
                languages: &[],
                cancel,
            }
            .run_with_item_filter::<rows::MagazineDependencies, T>(None, filter, inspect)
        })
    }

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
