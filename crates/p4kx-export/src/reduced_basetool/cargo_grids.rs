// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use crate::catalog_queries::equipment::{CapacityStatus, ComponentOwner, Facility};
use crate::catalog_queries::{DatasetAvailability, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_v3::facility_instance::FacilityConfiguration;

pub(super) const PATH: &str = "configurations.facilities";

#[derive(Default)]
pub(super) struct CargoGrids {
    by_record: BTreeMap<u32, Vec<Grid>>,
    availability: Option<DatasetAvailability>,
}

pub(super) struct Grid {
    facts: GridFacts,
    owner: ComponentOwner,
}

pub(super) struct GridFacts {
    pub(super) key: ComponentKey,
    pub(super) capacity: Option<Capacity>,
    pub(super) mining_only: Cell<bool>,
}

pub(super) struct Capacity {
    pub(super) scu: Cell<f64>,
    pub(super) status: CapacityStatus,
    pub(super) inventory_record_index: Option<u32>,
}

impl CargoGrids {
    pub(super) fn placement_availability(&self, count: usize) -> DatasetAvailability {
        if count == 0 {
            self.availability
                .unwrap_or(DatasetAvailability::Unavailable)
        } else {
            DatasetAvailability::Available
        }
    }

    pub(super) fn collect(
        parsed: &ParsedCatalogQueries<'_>,
        scratch: &Path,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, StagedCatalogTable), ReducedError> {
        let mut definitions = StagedCatalogRows::new(scratch, cancel);
        let mut grids = Self::default();
        let summary = parsed.visit_cargo_grid_owners(cancel, |_, snapshot| {
            for owner in snapshot.rows() {
                completeness::all_fields(
                    PATH,
                    snapshot.availability(),
                    &owner.dependency_coverage,
                )?;
                for facility in &owner.facilities {
                    grids.add(facility, &mut definitions)?;
                }
            }
            Ok::<_, ReducedError>(())
        })?;
        completeness::all_fields(PATH, summary.availability(), summary.coverage())?;
        let table = definitions.finish(summary.availability())?;
        let observed = grids.by_record.values().map(Vec::len).sum();
        completeness::rows(PATH, observed, table.len())?;
        grids.availability = Some(summary.availability());
        Ok((grids, table))
    }

    fn add(
        &mut self,
        facility: &Facility<'_>,
        definitions: &mut StagedCatalogRows<'_>,
    ) -> Result<(), ReducedError> {
        let owner = facility.owner();
        let key = ComponentKey::read(owner, PATH)?;
        let grid = facility
            .cargo_grid_fields()
            .ok_or(ReducedError::IncompleteScope(PATH))?;
        let capacity = grid.capacity();
        let owners = self.by_record.entry(key.record).or_default();
        if owners
            .last()
            .is_some_and(|previous| previous.facts.key.ordinal >= key.ordinal)
        {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        definitions.append(&Row {
            kind: GridKind::CargoGrid,
            owner: Component::from(owner),
            capacity: capacity.map(|capacity| GridCapacity {
                scu: capacity.scu().into(),
            }),
            mining_only: grid.mining_only().into(),
            invisible: grid.invisible().into(),
        })?;
        owners.push(Grid {
            facts: GridFacts {
                key,

                capacity: capacity.map(|capacity| Capacity {
                    scu: Cell::from(capacity.scu()).map(|value| *value),
                    status: capacity.status(),
                    inventory_record_index: capacity
                        .container_record_index()
                        .and_then(|value| value.value().copied()),
                }),
                mining_only: Cell::from(grid.mining_only()).map(|value| *value),
            },
            owner: owner.clone(),
        });
        Ok(())
    }

    pub(super) fn links(&self, record: u32) -> Vec<FacilityConfiguration> {
        self.by_record
            .get(&record)
            .into_iter()
            .flatten()
            .map(|grid| FacilityConfiguration::Facility(grid.owner.clone()))
            .collect()
    }

    pub(super) fn get(&self, key: ComponentKey) -> Option<&GridFacts> {
        self.by_record
            .get(&key.record)?
            .iter()
            .map(|grid| &grid.facts)
            .find(|grid| grid.key == key)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum GridKind {
    CargoGrid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'source> {
    kind: GridKind,
    #[serde(flatten)]
    owner: Component<'source>,
    capacity: Option<GridCapacity<'source>>,
    mining_only: Cell<&'source bool>,
    invisible: Cell<&'source bool>,
}

#[derive(Serialize)]
struct GridCapacity<'source> {
    scu: Cell<&'source f64>,
}
