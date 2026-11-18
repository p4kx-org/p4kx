// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use super::cargo_read::CargoGridOwner;
pub use crate::catalog_v3::catalog_facility::{
    CapacityStatus, CargoGrid, CargoGridCapacity, Facility,
};
pub use crate::catalog_v3::inventory_container_definition::InventoryContainerDefinition;
pub use crate::catalog_v3::inventory_container_type::InventoryContainerType;
pub use crate::catalog_v3::inventory_item_type_filter::InventoryItemTypeFilter;
pub use crate::catalog_v3::inventory_open_container::InventoryOpenContainer;
pub use crate::catalog_v3::inventory_owner_storage::InventoryOwnerStorage;

/// Canonical Cargo Grid configurations and the reached inventory definition of one physical owner.
pub type CargoGridSnapshot<'source> = super::EquipmentSnapshot<'source, CargoGridOwner<'source>>;

/// Supplies Cargo Grid configuration rows and their inventory dependency from one physical Record.
/// Publish consumer changes after outer success and the required coverage checks.
///
/// # Errors
/// Returns Archive, source-evidence or cancellation errors.
pub fn with_cargo_grids<T>(
    archive: &Archive,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CargoGridSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::CargoGrids, T>(
        archive,
        None,
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same canonical rows from an already decoded original Archive Entry.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_cargo_grids_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    record_index: u32,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CargoGridSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::CargoGrids, T>(
        archive,
        Some(entry),
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Supplies the selected physical owner's Cargo Grid rows through this session.
    ///
    /// # Errors
    /// Returns query, Archive-change or cancellation errors.
    pub fn with_cargo_grids<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CargoGridSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_cargo_grids(record_index, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies the selected physical owner's Cargo Grid rows through this parsed view.
    ///
    /// # Errors
    /// Returns query, Archive-change or cancellation errors.
    pub fn with_cargo_grids<T>(
        &self,
        record_index: u32,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CargoGridSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::CargoGrids, T>(
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
