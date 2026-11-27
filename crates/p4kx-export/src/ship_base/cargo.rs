// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::{EntityNode, LoadoutEntry};
use p4kx_extraction::{DataForge, DataForgeError, Guid, TreeBuilder};

use crate::loadout::ledger::Capture;

use super::model::{Cargo, CargoGrid, Source, SourceKind};
use crate::catalog_v3::cargo_source::round_capacity;
#[path = "cargo_admission.rs"]
mod cargo_admission;
use cargo_admission::CargoAdmission;

#[path = "cargo_container.rs"]
mod cargo_container;
use cargo_container::{ContainerInput, MeasuredCapacity};

#[path = "cargo_capacity_origin.rs"]
mod cargo_capacity_origin;
#[path = "cargo_observed.rs"]
mod cargo_observed;

use cargo_capacity_origin::CapacitySource;
pub(crate) use cargo_observed::Observation;
pub use cargo_observed::{ShipCargoMountSource, ShipCargoSource};

const CELL_METRES_NUMERATOR: u32 = 5;
const CELL_METRES_DENOMINATOR: f64 = 4.0;
const CELL_TOLERANCE: f64 = 1e-4;
#[expect(clippy::cast_precision_loss, reason = "a power of two is exact in f64")]
const CELL_COUNT_LIMIT: f64 = (1_u128 << u64::BITS) as f64;

struct Container<'source> {
    record_path: &'source str,
    capacity: MeasuredCapacity<'source>,
    origin: Option<CapacitySource>,
}

enum ContainerReadError {
    Missing,
    Unreadable {
        error: DataForgeError,
        origin: Option<CapacitySource>,
    },
}

impl ContainerReadError {
    const fn origin(&self) -> Option<&CapacitySource> {
        match self {
            Self::Missing => None,
            Self::Unreadable { origin, .. } => origin.as_ref(),
        }
    }

    fn for_guid(&self, guid: Guid) -> String {
        match self {
            Self::Missing => format!(
                "the inventory container {guid} does not exist: a dangling Reference in the game data"
            ),
            Self::Unreadable { error, .. } => {
                format!("cannot read the inventory container {guid}: {error}")
            }
        }
    }
}

/// Finds the cargo grids of default Loadouts and reads their capacity, keeping each
/// `InventoryContainer` Record it has read.
pub(crate) struct CargoReader<'f> {
    forge: &'f DataForge<'f>,
    containers: HashMap<Guid, Result<Container<'f>, ContainerReadError>>,
}

impl<'f> CargoReader<'f> {
    pub(crate) fn new(forge: &'f DataForge<'f>) -> Self {
        Self {
            forge,
            containers: HashMap::new(),
        }
    }

    /// Returns the cargo grids among the Loadout `entries` of a ship and their sum. Grids of
    /// docked vehicles are not in the tree, templates are left out.
    pub(crate) fn read(
        &mut self,
        entries: &[LoadoutEntry<'_>],
        trees: &mut TreeBuilder<'f>,
    ) -> Cargo {
        self.read_inner(entries, trees, None)
    }

    pub(crate) fn read_observed(
        &mut self,
        entries: &[LoadoutEntry<'_>],
        trees: &mut TreeBuilder<'f>,
        observation: &mut Observation<'_, 'f>,
    ) -> Cargo {
        self.read_inner(entries, trees, Some(observation))
    }

    fn read_inner(
        &mut self,
        entries: &[LoadoutEntry<'_>],
        trees: &mut TreeBuilder<'f>,
        mut observation: Option<&mut Observation<'_, 'f>>,
    ) -> Cargo {
        let mut grids = Vec::new();
        for entry in entries {
            let Some(node) = entry.entity() else {
                continue;
            };
            let admission = CargoAdmission::of(node);
            if let Some(observation) = observation.as_deref_mut() {
                observation.admission(node, admission);
            }
            if matches!(admission, CargoAdmission::Grid) {
                grids.push(self.grid(entry, node, trees, observation.as_deref_mut()));
            }
        }
        let with_issue = grids.iter().filter(|grid| grid.scu.is_none()).count();
        let known = grids.is_empty() || grids.len() > with_issue;
        let scu = grids.iter().filter_map(|grid| grid.scu).sum::<f64>();
        Cargo {
            scu: known.then(|| round_capacity(scu)),
            cells: known.then(|| total_cells(&grids)).flatten(),
            grids_with_issue: with_issue,
            grids,
        }
    }

    fn grid(
        &mut self,
        entry: &LoadoutEntry<'_>,
        node: &EntityNode,
        trees: &mut TreeBuilder<'f>,
        mut observation: Option<&mut Observation<'_, 'f>>,
    ) -> CargoGrid {
        let mut grid = CargoGrid {
            port: entry.path().to_owned(),
            class_name: node.class_name().to_owned(),
            guid: node.guid(),
            container: node.inventory_container(),
            shape: None,
            scu: None,
            cells: None,
            dimensions: None,
            source: None,
            issue: None,
        };
        let origin = if let Some(container) = grid.container {
            let capture = observation
                .as_ref()
                .map(|observation| observation.capture());
            match self.container(container, trees, capture) {
                Err(error) => {
                    grid.issue = Some(error.for_guid(container));
                    error.origin()
                }
                Ok(read) => {
                    fill(&mut grid, read);
                    read.origin.as_ref()
                }
            }
        } else {
            grid.issue = Some("the Entity names no inventory container".to_owned());
            None
        };
        if let Some(observation) = &mut observation {
            observation.mount(entry, node, &grid, origin);
        }
        grid
    }

    fn container(
        &mut self,
        guid: Guid,
        trees: &mut TreeBuilder<'f>,
        capture: Option<&Capture<'f>>,
    ) -> &Result<Container<'f>, ContainerReadError> {
        let forge = self.forge;
        self.containers.entry(guid).or_insert_with(|| {
            let record = forge
                .record_by_guid(guid)
                .ok_or(ContainerReadError::Missing)?;
            let tree = trees
                .build(&record)
                .map_err(|error| ContainerReadError::Unreadable {
                    error,
                    origin: capture
                        .and_then(|capture| cargo_observed::unreadable(capture, &record)),
                })?;
            let observed =
                capture.and_then(|capture| cargo_observed::capacity(capture, &record, &tree));
            let (capacity, origin) = observed.map_or_else(
                || {
                    (
                        ContainerInput::select(tree.root(), &mut |_, _, _, _| {}).measure(),
                        None,
                    )
                },
                |(capacity, origin)| (capacity, Some(origin)),
            );
            Ok(Container {
                record_path: tree.record_path(),
                capacity,
                origin,
            })
        })
    }
}

fn fill(grid: &mut CargoGrid, container: &Container<'_>) {
    let field = |path| Source {
        kind: SourceKind::Record,
        file: container.record_path.to_owned(),
        field: path,
    };
    match &container.capacity {
        MeasuredCapacity::Open { dimensions, scu } => {
            grid.shape = Some("open");
            grid.dimensions = Some(*dimensions);
            grid.scu = Some(round_capacity(*scu));
            grid.cells = cells(*dimensions);
            grid.source = Some(field("interiorDimensions".to_owned()));
        }
        MeasuredCapacity::Closed { scu, unit } => {
            grid.shape = Some("closed");
            grid.scu = Some(round_capacity(*scu));
            grid.source = Some(field(format!(
                "inventoryType/capacity/{}/{}",
                unit.source_type(),
                unit.amount_field()
            )));
        }
        MeasuredCapacity::Unavailable(reason) => grid.issue = Some(reason.to_string()),
    }
}

fn cells(dimensions: [f64; 3]) -> Option<u64> {
    let counts = dimensions.map(axis_cells);
    if counts.contains(&Some(0)) {
        return Some(0);
    }
    counts
        .into_iter()
        .try_fold(1_u64, |total, count| total.checked_mul(count?))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the bounded nonnegative scaled length is split at its integer boundary"
)]
fn axis_cells(side: f64) -> Option<u64> {
    let scaled = side * CELL_METRES_DENOMINATOR;
    let numerator = f64::from(CELL_METRES_NUMERATOR);
    if scaled >= CELL_COUNT_LIMIT * numerator {
        return None;
    }
    // Integer division retains whole cells that the floating quotient cannot distinguish.
    let whole = scaled as u128;
    let divisor = u128::from(CELL_METRES_NUMERATOR);
    let remainder = u32::try_from(whole % divisor).ok()?;
    let fraction = f64::from(remainder) + scaled.fract();
    let within_tolerance = fraction >= numerator * (1.0 - CELL_TOLERANCE);
    u64::try_from(whole / divisor + u128::from(within_tolerance)).ok()
}

fn total_cells(grids: &[CargoGrid]) -> Option<u64> {
    grids.iter().try_fold(0_u64, |total, grid| {
        let cells = if grid.dimensions.is_some() {
            grid.cells?
        } else {
            0
        };
        total.checked_add(cells)
    })
}

/// Rounds a sum to four decimals.
pub(super) fn rounded(value: f64) -> f64 {
    round_capacity(value)
}
