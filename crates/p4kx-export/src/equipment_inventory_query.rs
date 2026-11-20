// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{Family, Projector};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::catalog_v3::inventory_container_definition::{self, InventoryContainerDefinition};
use crate::record_observation::RecordObservation;

struct InventoryContainerRows;

impl Projector for InventoryContainerRows {
    type Row<'source> = InventoryContainerDefinition<'source>;
    const FAMILY: Family = Family::InventoryContainers;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &AlignmentOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let (row, notes) =
            inventory_container_definition::read(tree, projection, admission, cancel)?;
        Ok(ScopedSourceRead {
            value: vec![row.into_definition()],
            notes,
            selection_complete: true,
        })
    }
}

record_query::unlocalized!(
    InventoryContainerRows,
    InventoryContainerDefinition,
    /// Selected original inventory container definitions with their source evidence.
    InventoryContainerSnapshot,
    with_inventory_containers,
    with_inventory_containers_from
);
