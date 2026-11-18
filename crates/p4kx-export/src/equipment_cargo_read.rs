// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_facility::Facility;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::catalog_v3::inventory_container_definition::{
    self, InventoryContainerDefinition, PreparedInventoryContainer,
};
use crate::catalog_v3::inventory_owner_storage::InventoryOwnerStorage;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

/// Cargo Grid facilities, storage configurations and reached container for one physical owner.
#[derive(Debug)]
#[non_exhaustive]
pub struct CargoGridOwner<'source> {
    /// Canonical Cargo Grid rows in original component order.
    pub facilities: Vec<Facility<'source>>,
    /// Canonical item storage configurations on this owner.
    pub inventory_configurations: Vec<InventoryOwnerStorage<'source>>,
    /// The one physical inventory definition admitted to the capacity calculation.
    pub inventory_container: Option<InventoryContainerDefinition<'source>>,
    /// Body and field coverage for the admitted inventory definition.
    /// Consult the original Reference and capacity status for rejected targets.
    pub dependency_coverage: CatalogRecordCoverage,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<CargoGridOwner<'source>>>, CatalogError> {
    let mut rows = catalog_entity_configurations::read_cargo_configurations(
        tree, projection, admission, options, cancel,
    )?;
    if !rows.value.has_cargo_grids() {
        return Ok(rows.map(|_| Vec::new()));
    }
    projection.original().check_forge(forge)?;
    let target = rows.value.cargo_container_index(forge)?;
    let mut dependency_coverage = CatalogRecordCoverage::default();
    let container = match target {
        Some(index) => read_container(forge, index, projection, cancel, &mut dependency_coverage)?,
        None => None,
    };
    rows.value.resolve_cargo_capacities(
        forge,
        container.as_slice(),
        &projection.origins().uses,
        None,
        cancel,
    )?;
    Ok(rows.map(|rows| {
        let (facilities, inventory_configurations) = rows.into_cargo_rows();
        vec![CargoGridOwner {
            facilities,
            inventory_configurations,
            inventory_container: container.map(PreparedInventoryContainer::into_definition),
            dependency_coverage,
        }]
    }))
}

fn read_container<'source>(
    forge: &'source DataForge<'source>,
    index: u32,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Option<PreparedInventoryContainer<'source>>, CatalogError> {
    checkpoint(cancel)?;
    let input = projection.original();
    let record = forge
        .record(index)
        .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index })?;
    let tree = match record.tree() {
        Ok(tree) => tree,
        Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
        Err(_) => {
            coverage.unreadable_bodies.push(index);
            return Ok(None);
        }
    };
    let admission = RecordObservation::capture(input, &record)?;
    let (container, notes) =
        inventory_container_definition::read(&tree, projection, &admission, cancel)?;
    if notes
        .iter()
        .any(|note| note.description().issue.blocks_projection())
    {
        coverage.incomplete_fields.push(index);
    }
    Ok(Some(container))
}
