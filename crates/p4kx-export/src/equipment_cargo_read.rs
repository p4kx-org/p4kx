// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_entity_configurations::{self, FacilitySelection};
use crate::catalog_v3::catalog_facility::Facility;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::catalog_v3::inventory_container_definition::{
    self, InventoryContainerDefinition, PreparedInventoryContainer,
};
use crate::catalog_v3::inventory_owner_storage::InventoryOwnerStorage;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

/// Canonical facilities and any capacity dependencies on one physical owner.
#[derive(Debug)]
#[non_exhaustive]
pub struct FacilityOwner<'source> {
    /// Canonical facility rows in original component order.
    pub facilities: Vec<Facility<'source>>,
    /// Co-owned item storage configurations consulted for Cargo Grid capacity.
    pub inventory_configurations: Vec<InventoryOwnerStorage<'source>>,
    /// The one physical inventory definition admitted to the capacity calculation.
    pub inventory_container: Option<InventoryContainerDefinition<'source>>,
    /// Body and field coverage for the admitted inventory definition.
    /// Consult the original Reference and capacity status for rejected targets.
    pub dependency_coverage: CatalogRecordCoverage,
}

/// A facility owner selected through the Cargo Grid query.
pub type CargoGridOwner<'source> = FacilityOwner<'source>;

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    options: &AlignmentOptions<'_>,
    selection: FacilitySelection,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<FacilityOwner<'source>>>, CatalogError> {
    let mut rows = catalog_entity_configurations::read_facility_configurations(
        tree, projection, admission, options, selection, cancel,
    )?;
    if !rows.value.has_facilities() {
        return Ok(rows.map(|_| Vec::new()));
    }
    let mut dependency_coverage = CatalogRecordCoverage::default();
    let mut container = None;
    if rows.value.has_cargo_grids() {
        projection.original().check_forge(forge)?;
        if let Some(index) = rows.value.cargo_container_index(forge)? {
            container = read_container(forge, index, projection, cancel, &mut dependency_coverage)?;
        }
        rows.value.resolve_cargo_capacities(
            forge,
            container.as_slice(),
            &projection.origins().uses,
            None,
            cancel,
        )?;
    }
    Ok(rows.map(|rows| {
        let (facilities, inventory_configurations) = rows.into_cargo_rows();
        vec![FacilityOwner {
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
