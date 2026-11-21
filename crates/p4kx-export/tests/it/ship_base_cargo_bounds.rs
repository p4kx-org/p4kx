// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{ShipBaseOptions, build_ship_base};
use p4kx_extraction::{DataForge, DataForgeEntry, Value};
use p4kx_testkit::{
    DataForgeBuilder, TestCapacity, TestEntity, TestLoadout, TestLoadoutEntry, TestPort,
    TestRecord, closed_container_record, entity_schema, open_container_record,
};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

const SHIP_BASE: u128 = 0x1063_0600;
const GRID_BASE: u128 = 0x1063_0700;
const STORAGE_BASE: u128 = 0x1063_0800;
const FIRST_PORT: &str = "grid_a";
const SECOND_PORT: &str = "grid_b";
const ONE_PORT: &[&str] = &[FIRST_PORT];
const TWO_PORTS: &[&str] = &[FIRST_PORT, SECOND_PORT];
const CELL: f32 = 1.25;

struct Case {
    name: &'static str,
    input: Input,
    ports: &'static [&'static str],
    expected_scu: Option<f64>,
    expected_cells: Option<u64>,
    expected_grid_cells: Option<u64>,
}

enum Input {
    Open([f32; 3]),
    Closed(TestCapacity),
}

impl Case {
    const fn open(
        name: &'static str,
        side: f32,
        expected_scu: Option<f64>,
        expected_cells: Option<u64>,
    ) -> Self {
        Self {
            name,
            input: Input::Open([side, CELL, CELL]),
            ports: ONE_PORT,
            expected_scu,
            expected_cells,
            expected_grid_cells: expected_cells,
        }
    }

    const fn closed(name: &'static str, capacity: TestCapacity, expected_scu: Option<f64>) -> Self {
        Self {
            name,
            input: Input::Closed(capacity),
            ports: ONE_PORT,
            expected_scu,
            expected_cells: if expected_scu.is_some() {
                Some(0)
            } else {
                None
            },
            expected_grid_cells: None,
        }
    }
}

#[test]
fn shipbase_accepts_finite_source_lengths_and_capacities_above_old_limits() -> TestResult {
    verify(&[
        Case::open("OpenExact", 1000.0, Some(800.0), Some(800)),
        Case::open("OpenNext", 1000.0_f32.next_up(), Some(800.0), Some(800)),
        Case::open("OpenLarge", 1250.0, Some(1000.0), Some(1000)),
        Case::closed("ClosedExact", TestCapacity::Standard(1e9), Some(1e9)),
        Case::closed(
            "ClosedNext",
            TestCapacity::Standard(1e9_f32.next_up()),
            Some(1_000_000_064.0),
        ),
    ])
}

#[test]
fn shipbase_preserves_scu_when_a_cell_axis_product_or_aggregate_exceeds_u64() -> TestResult {
    let half = 2.0_f32.powi(63);
    let full = 2.0_f64.powi(64);
    let side = CELL * 2.0_f32.powi(32);
    verify(&[
        Case::open("OpenAxisOverflow", CELL * half * 2.0, Some(full), None),
        Case {
            name: "OpenProductOverflow",
            input: Input::Open([side, side, CELL]),
            ..Case::open("OpenProductOverflow", 0.0, Some(full), None)
        },
        Case {
            ports: TWO_PORTS,
            expected_grid_cells: Some(1_u64 << 63),
            ..Case::open("OpenTotalOverflow", CELL * half, Some(full), None)
        },
        Case {
            name: "OpenZeroCellProduct",
            input: Input::Open([CELL * half * 2.0, CELL, CELL / 2.0]),
            ..Case::open("OpenZeroCellProduct", 0.0, Some(full / 2.0), Some(0))
        },
    ])
}

#[test]
fn shipbase_preserves_large_exact_cell_counts_and_the_cell_tolerance() -> TestResult {
    let last_side = (CELL * 2.0_f32.powi(64)).next_down();
    let capacity = f64::from(last_side) / f64::from(CELL);
    verify(&[
        Case::open(
            "OpenLastSide",
            last_side,
            Some(capacity),
            Some(18_446_742_314_490_947_174),
        ),
        Case::open("OpenWithinTolerance", 1.2499, Some(0.9999), Some(1)),
        Case::open("OpenBelowTolerance", 1.2498, Some(0.9998), Some(0)),
    ])
}

#[test]
fn shipbase_scales_original_signed_integer_units_before_rounding() -> TestResult {
    verify(&[
        Case::closed(
            "CentiExact",
            TestCapacity::Centi(16_777_217),
            Some(167_772.17),
        ),
        Case::closed(
            "MicroExact",
            TestCapacity::Micro(2_147_483_580),
            Some(2_147.483_6),
        ),
    ])
}

#[test]
fn shipbase_keeps_invalid_dimensions_and_capacities_unreadable() -> TestResult {
    verify(&[
        Case::open("OpenZero", 0.0, None, None),
        Case::open("OpenNegative", -1.0, None, None),
        Case::open("OpenNonfinite", f32::INFINITY, None, None),
        Case::closed("ClosedZero", TestCapacity::Standard(0.0), Some(0.0)),
        Case::closed("ClosedNegative", TestCapacity::Standard(-1.0), None),
        Case::closed(
            "ClosedNonfinite",
            TestCapacity::Standard(f32::INFINITY),
            None,
        ),
        Case::closed("CentiNegative", TestCapacity::Centi(-1), None),
        Case::closed("MicroNegative", TestCapacity::Micro(-1), None),
    ])
}

fn verify(cases: &[Case]) -> TestResult {
    let mut database = entity_schema(DataForgeBuilder::new().version(8));
    for (index, case) in (0_u32..).zip(cases) {
        let [ship, grid, storage] = records(u128::from(index), case);
        database = database.record(ship).record(grid).record(storage);
    }
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let dcb = DataForgeEntry::read(&archive)?;
    let forge = dcb.open()?;
    let bases = build_ship_base(&archive, &ShipBaseOptions::new(UNIX_EPOCH))?;
    for (index, case) in (0_u32..).zip(cases) {
        verify_source(&forge, index * 3 + 2, &case.input)?;
        let guid = common::guid(SHIP_BASE + u128::from(index));
        let row = bases
            .ships()
            .iter()
            .find(|ship| ship.guid.to_string() == guid)
            .ok_or("actual synthetic ShipBase row")?;
        assert_eq!(
            row.cargo.scu, case.expected_scu,
            "capacity of {}",
            case.name
        );
        assert_eq!(
            row.cargo.cells, case.expected_cells,
            "cells of {}",
            case.name
        );
        let count = case.ports.len();
        assert_eq!(
            row.cargo.grids.len(),
            count,
            "physical grids of {}",
            case.name
        );
        assert_eq!(
            row.cargo.grids_with_issue,
            if case.expected_scu.is_some() {
                0
            } else {
                count
            },
            "capacity issues of {}",
            case.name
        );
        for grid in &row.cargo.grids {
            assert_eq!(
                grid.cells, case.expected_grid_cells,
                "local cells of {}",
                case.name
            );
            assert_eq!(
                grid.issue.is_none(),
                case.expected_scu.is_some(),
                "local capacity of {}",
                case.name
            );
        }
    }
    Ok(())
}

fn verify_source(forge: &DataForge<'_>, record: u32, input: &Input) -> TestResult {
    let tree = forge
        .record(record)
        .ok_or("original InventoryContainer record")?
        .tree()?;
    match *input {
        Input::Open(expected) => {
            let Some(Value::Struct(dimensions)) = tree.field("interiorDimensions") else {
                return Err("original Vec3".into());
            };
            for (field, expected) in ["x", "y", "z"].into_iter().zip(expected) {
                assert!(
                    matches!(dimensions.field(field), Some(Value::Single(actual)) if actual.to_bits() == expected.to_bits()),
                    "original Single dimension {field}"
                );
            }
        }
        Input::Closed(capacity) => {
            let Some(Value::Struct(kind)) = tree.field("inventoryType") else {
                return Err("original InventoryClosedContainerType".into());
            };
            let Some(Value::Struct(amount)) = kind.field("capacity") else {
                return Err("original cargo capacity unit".into());
            };
            match capacity {
                TestCapacity::Standard(expected) => assert!(
                    matches!(amount.field("standardCargoUnits"), Some(Value::Single(actual)) if actual.to_bits() == expected.to_bits()),
                    "original Single standard SCU"
                ),
                TestCapacity::Centi(expected) => assert!(
                    matches!(amount.field("centiSCU"), Some(Value::Int32(actual)) if *actual == expected),
                    "original Int32 centi SCU"
                ),
                TestCapacity::Micro(expected) => assert!(
                    matches!(amount.field("microSCU"), Some(Value::Int32(actual)) if *actual == expected),
                    "original Int32 micro SCU"
                ),
            }
        }
    }
    Ok(())
}

fn records(index: u128, case: &Case) -> [TestRecord; 3] {
    let mut ship = TestEntity::new(
        SHIP_BASE + index,
        format!("Fixture{}", case.name),
        format!("libs/foundry/records/entities/spaceships/{}.xml", case.name),
    )
    .vehicle("", "");
    let mut mounts = Vec::new();
    for &port in case.ports {
        ship = ship.port(TestPort::new(port));
        mounts.push(TestLoadoutEntry::reference(port, GRID_BASE + index));
    }
    let grid = TestEntity::new(
        GRID_BASE + index,
        format!("FixtureGrid{}", case.name),
        format!(
            "libs/foundry/records/entities/scitem/fixture/grid_{}.xml",
            case.name
        ),
    )
    .attach_type("CargoGrid")
    .container(STORAGE_BASE + index)
    .record();
    let storage = match case.input {
        Input::Open(dimensions) => {
            open_container_record(STORAGE_BASE + index, case.name, dimensions)
        }
        Input::Closed(capacity) => {
            closed_container_record(STORAGE_BASE + index, case.name, capacity)
        }
    };
    [
        ship.loadout(TestLoadout::Manual(mounts)).record(),
        grid,
        storage,
    ]
}
