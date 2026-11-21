// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Columns that share the first steps of their path give the same rows, error values, unresolved
//! counts and budget use whether those steps are walked once or once per column.

use std::error::Error;

use crate::fleet::database as fleet_database;
use crate::ships::{database, schema};
use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{MAX_PATH_VALUES, Query};
use p4kx_testkit::{TestInstance, TestRecord, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn outcome(forge: &DataForge<'_>, text: &str, share: bool) -> Result<Vec<String>, Box<dyn Error>> {
    let mut compiled = Query::parse(text)?.compile(forge)?;
    if !share {
        compiled = compiled.without_shared_prefixes();
    }
    let run = compiled.run()?;
    Ok(run
        .rows(0..run.len())
        .iter()
        .map(|row| {
            format!(
                "{} {:?} {}",
                row.record_path(),
                row.cells(),
                row.unresolved_references()
            )
        })
        .collect())
}

fn assert_same(forge: &DataForge<'_>, text: &str) -> TestResult {
    let shared = outcome(forge, text, true)?;
    let alone = outcome(forge, text, false)?;
    assert!(!shared.is_empty(), "{text} selects Records");
    assert_eq!(shared, alone, "{text}");
    Ok(())
}

fn words(first: u32, second: u32) -> TestValue {
    TestValue::Raw([first.to_le_bytes(), second.to_le_bytes()].concat())
}

fn weapon(name: &str, modes: usize) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Weapon")
            .value("name", TestValue::text(name))
            .value(
                "modes",
                TestValue::Array(
                    (0..modes)
                        .map(|mode| TestValue::Int32(i32::try_from(mode).unwrap_or(0)))
                        .collect(),
                ),
            ),
    )
}

fn tags(count: usize) -> TestValue {
    TestValue::Array(
        (0..count)
            .map(|index| TestValue::Int32(i32::try_from(index).unwrap_or(0)))
            .collect(),
    )
}

#[test]
fn components_of_one_type_read_the_same_shared_or_alone() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    for text in [
        "from Ship select Components[*].name, Components[*].modes[*], Components[*].modes[0]",
        "from Ship select Components[Weapon].name, Components[Weapon].damage, Components[Weapon].modes[*]",
        "from Ship select Components[Weapon].name, mass, Components[Weapon].damage, tags[*], \
         Components[Shield].hp, Components[Shield].name, Components[Weapon].modes[1]",
        "from Ship select hardpoints[*].x, hardpoints[*].y, position.x, hardpoints[1].y",
        "from Ship select Components[*].name, Components[*].modes[*] \
         where Components[*].name = \"Laser\"",
        "from Ship select tags[*], tags[*], tags[0], tags[*]",
    ] {
        assert_same(&forge, text)?;
    }
    Ok(())
}

#[test]
fn followed_links_count_the_same_unresolved_ones_shared_or_alone() -> TestResult {
    let bytes = fleet_database();
    let forge = DataForge::open(&bytes)?;

    for text in [
        "from Ship select escorts[*]->name, escorts[*]->mass, escorts[*]->escorts[*]->name",
        "from Ship select escorts[*]->name, maker->name, escorts[*]->mass, escorts[0]->mass",
        "from Ship select escorts[*]->escorts[*]->name, escorts[*]->escorts[*]->mass, mass",
    ] {
        assert_same(&forge, text)?;
    }
    Ok(())
}

#[test]
fn unreadable_steps_give_the_same_error_values() -> TestResult {
    let bytes = schema()
        .record(
            TestRecord::new(8, "Ship", "Ship.Broken", "libs/ships/broken.xml")
                .value("Components", words(3, 1000))
                .value("tags", words(2, 1_000_000)),
        )
        .record(
            TestRecord::new(9, "Ship", "Ship.Odd", "libs/ships/odd.xml").value(
                "Components",
                TestValue::Array(vec![words(77, 0), weapon("Laser", 2), TestValue::Null]),
            ),
        )
        .record(
            TestRecord::new(10, "Ship", "Ship.Fine", "libs/ships/fine.xml")
                .value("Components", TestValue::Array(vec![weapon("Cannon", 3)])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;

    for text in [
        "from Ship select Components[*].name, Components[*].modes[*], tags[*], tags[*]",
        "from Ship select Components[Weapon].name, Components[Weapon].modes[*], \
         Components[Weapon].modes[0]",
        "from Ship select Components[*].name, Components[*].modes[1]",
    ] {
        assert_same(&forge, text)?;
    }
    Ok(())
}

#[test]
fn the_budget_runs_out_at_the_same_column_shared_or_alone() -> TestResult {
    let components = |modes: usize| {
        TestValue::Array(
            (0..64)
                .map(|index| weapon(&format!("W{index}"), modes))
                .collect(),
        )
    };
    let per_weapon = MAX_PATH_VALUES / 64;
    let bytes = schema()
        .record(
            TestRecord::new(8, "Station", "Station.Big", "libs/stations/big.xml")
                .value("tags", tags(MAX_PATH_VALUES + 1)),
        )
        .record(
            TestRecord::new(9, "Station", "Station.Half", "libs/stations/half.xml")
                .value("tags", tags(MAX_PATH_VALUES / 2 + 1)),
        )
        .record(
            TestRecord::new(10, "Station", "Station.Exact", "libs/stations/exact.xml")
                .value("tags", tags(MAX_PATH_VALUES / 2)),
        )
        .record(
            TestRecord::new(11, "Station", "Station.Small", "libs/stations/small.xml")
                .value("tags", tags(3)),
        )
        .record(
            TestRecord::new(12, "Ship", "Ship.Fits", "libs/ships/fits.xml")
                .value("Components", components(per_weapon - 1)),
        )
        .record(
            TestRecord::new(13, "Ship", "Ship.Wide", "libs/ships/wide.xml")
                .value("Components", components(per_weapon)),
        )
        .build();
    let forge = DataForge::open(&bytes)?;

    for text in [
        "from Station select tags[*], tags[*], tags[*]",
        "from Station select tags[4000], tags[*], tags[*], tags[1], tags[*]",
        "from Station select tags[*], tags[2], tags[*]",
        "from Ship select Components[*].name, Components[*].modes[*], Components[*].modes[*]",
        "from Ship select Components[*].modes[*], Components[*].name, Components[*].name, \
         Components[*].modes[0]",
    ] {
        assert_same(&forge, text)?;
    }
    Ok(())
}

#[test]
fn the_shared_walk_stops_where_a_column_alone_would() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    for text in [
        "from Ship select Components[*].name, Components[*].name",
        "from Ship select Components[Weapon].modes[0], Components[Weapon].modes[1], \
         Components[Weapon].name",
    ] {
        assert_same(&forge, text)?;
    }
    Ok(())
}

type Groups = Vec<(Vec<usize>, usize)>;

fn groups_of(forge: &DataForge<'_>, text: &str) -> Result<Groups, Box<dyn Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let mut groups: Vec<_> = compiled
        .shared_prefixes()
        .into_iter()
        .map(|prefix| (prefix.columns.to_vec(), prefix.steps))
        .collect();
    groups.sort();
    Ok(groups)
}

#[test]
fn a_benchmark_like_query_forms_the_expected_groups() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let text = "from Ship select Components[*].name, Components[*].modes[*], \
                Components[*].modes[0], mass, hardpoints[*].x, hardpoints[*].y, tags[*]";

    assert_eq!(
        groups_of(&forge, text)?,
        vec![(vec![0, 1, 2], 1), (vec![4, 5], 1)]
    );
    assert_same(&forge, text)?;
    Ok(())
}

#[test]
fn the_groups_do_not_depend_on_the_column_order() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let text = "from Ship select Components[*].modes[*], hardpoints[*].y, Components[*].name, \
                tags[*], Components[*].modes[0], hardpoints[*].x, mass";

    assert_eq!(
        groups_of(&forge, text)?,
        vec![(vec![0, 2, 4], 1), (vec![1, 5], 1)]
    );
    assert_same(&forge, text)?;
    Ok(())
}

#[test]
fn a_column_that_leaves_early_does_not_shorten_the_prefix_of_the_others() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let text = "from Ship select Components[*].name, Components[*].modes[*], \
                Components[*].modes[*], Components[*].modes[0]";

    assert_eq!(
        groups_of(&forge, text)?,
        vec![(vec![0, 3], 1), (vec![1, 2], 2)]
    );
    assert_same(&forge, text)?;
    Ok(())
}
