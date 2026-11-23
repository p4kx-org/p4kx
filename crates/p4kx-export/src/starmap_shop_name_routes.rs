// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use super::{Limits, PlacementScope, PlacementStorage, Placements};
use crate::CatalogError;
use crate::catalog_queries::starmap_name_owners::AllStarmapNames;
use crate::catalog_read::original::EntryCapture;

impl Placements {
    pub(crate) fn read_for_shop_names(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        limits: Limits,
        capture: &EntryCapture<'_>,
        names: &AllStarmapNames,
    ) -> Result<Self, CatalogError> {
        Self::read_scoped(
            archive,
            cancel,
            limits,
            capture,
            PlacementScope::ArchiveInventory,
            None,
            PlacementStorage::ShopNames(names.descriptor_targets()),
        )
    }
}
