// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use p4kx_extraction::{CancelToken, SourcedValue};

use super::{GridRead, LocationPhysicsGrid};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::catalog_rows::spool::{RowSpool, RowSpoolWriter};
use crate::catalog_table::DatasetAvailability;
use crate::starmap_walk::SystemRead;
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(crate) struct GridObservations {
    rows: BTreeMap<PhysicalEntryRef, Arc<LocationPhysicsGrid>>,
    pub(crate) unavailable: bool,
}

impl GridObservations {
    pub(crate) fn record(
        &mut self,
        owner: &SourcedValue<PhysicalEntryRef>,
        grid: &GridRead,
    ) -> Result<(), CatalogError> {
        match grid {
            GridRead::Absent => Ok(()),
            GridRead::Unavailable => {
                self.unavailable = true;
                Ok(())
            }
            GridRead::Present(row) => {
                let physical = owner
                    .value()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                if row.container.value() != Some(physical) || row.descriptor.value().is_none() {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                insert(&mut self.rows, physical, row)
            }
        }
    }

    pub(crate) fn into_rows(self) -> Vec<Arc<LocationPhysicsGrid>> {
        self.rows.into_values().collect()
    }
}

fn insert(
    rows: &mut BTreeMap<PhysicalEntryRef, Arc<LocationPhysicsGrid>>,
    owner: &PhysicalEntryRef,
    row: &Arc<LocationPhysicsGrid>,
) -> Result<(), CatalogError> {
    match rows.get(owner) {
        Some(previous) if !Arc::ptr_eq(previous, row) => {
            Err(ExportAccountingError::ConflictingSourceDatum.into())
        }
        Some(_) => Ok(()),
        None => {
            rows.insert(owner.clone(), Arc::clone(row));
            Ok(())
        }
    }
}

pub(crate) struct GridSpool {
    pub(crate) spool: Option<RowSpool>,
    pub(crate) availability: DatasetAvailability,
}

pub(crate) fn write(
    systems: &[SystemRead],
    directory: &Path,
    cancel: Option<&CancelToken>,
) -> Result<GridSpool, CatalogError> {
    let mut rows = BTreeMap::new();
    for read in systems {
        for (owner, row) in &read.location_physics.rows {
            insert(&mut rows, owner, row)?;
        }
    }
    let complete = systems.iter().all(|read| {
        !read.location_physics.unavailable
            && !read.routes_incomplete
            && read.unreadable == 0
            && read.missing == 0
            && read.malformed == 0
    });
    let availability = availability(complete, !rows.is_empty());
    checkpoint(cancel)?;
    let spool = if rows.is_empty() {
        None
    } else {
        let mut writer = RowSpoolWriter::new(directory)?;
        for row in rows.into_values() {
            writer.append(row.as_ref(), cancel)?;
        }
        Some(writer.finish(cancel)?)
    };
    Ok(GridSpool {
        spool,
        availability,
    })
}

pub(crate) const fn availability(complete: bool, present: bool) -> DatasetAvailability {
    match (complete, present) {
        (_, true) => DatasetAvailability::Available,
        (true, false) => DatasetAvailability::Absent,
        (false, false) => DatasetAvailability::Unavailable,
    }
}
