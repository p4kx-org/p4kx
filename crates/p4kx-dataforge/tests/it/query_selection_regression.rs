// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Complete predicate selection and its distinction from missing values and projected cells.

use std::error::Error;

use p4kx_dataforge::query::{MAX_PATH_VALUES, Query};
use p4kx_dataforge::{DataForge, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn database(count: usize, hit: Option<usize>) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Station").array("tags", TestType::Int32))
        .structure(TestStruct::new("Other").field("marker", TestType::Int32))
        .record(
            TestRecord::new(1, "Station", "Station.Sample", "synthetic/station.xml").value(
                "tags",
                TestValue::Array(
                    (0..count)
                        .map(|index| TestValue::Int32(i32::from(hit == Some(index))))
                        .collect(),
                ),
            ),
        )
        .record(TestRecord::new(
            2,
            "Other",
            "Other.Sample",
            "synthetic/other.xml",
        ))
        .build()
}

fn selected(forge: &DataForge<'_>, text: &str) -> Result<Vec<u32>, Box<dyn Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    Ok(run.indexes().to_vec())
}

#[test]
fn an_early_witness_is_selected_in_4096_and_4097_elements() -> TestResult {
    for count in [4096, 4097] {
        let bytes = database(count, Some(1));
        let forge = DataForge::open(&bytes)?;
        assert_eq!(
            selected(&forge, "from Station where tags[1] = 1")?,
            [0],
            "direct positive control for {count} elements"
        );
        assert_eq!(
            selected(&forge, "from Station where tags[*] = 1")?,
            [0],
            "an early witness in {count} elements"
        );
    }
    Ok(())
}

#[test]
fn a_late_witness_is_selected_in_4096_and_4097_elements() -> TestResult {
    for (count, last) in [(4096, 4095), (4097, 4096)] {
        let bytes = database(count, Some(last));
        let forge = DataForge::open(&bytes)?;
        assert_eq!(
            selected(&forge, &format!("from Station where tags[{last}] = 1"))?,
            [0],
            "direct positive control for the last of {count} elements"
        );
        assert_eq!(
            selected(&forge, "from Station where tags[*] = 1")?,
            [0],
            "a late witness in {count} elements"
        );
    }
    Ok(())
}

#[test]
fn a_complete_no_match_and_its_negation_are_distinct() -> TestResult {
    let bytes = database(4097, None);
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        selected(&forge, "from Station where tags[4096] = 0")?,
        [0],
        "the last element exists and is readable"
    );
    assert!(
        selected(&forge, "from Station where tags[*] = 1")?.is_empty(),
        "a complete scan finds no witness"
    );
    assert_eq!(
        selected(&forge, "from Station where not tags[*] = 1")?,
        [0],
        "a complete false comparison negates to true"
    );
    Ok(())
}

#[test]
fn a_missing_field_and_an_out_of_range_index_remain_unknown() -> TestResult {
    let bytes = database(4097, None);
    let forge = DataForge::open(&bytes)?;
    for condition in ["tags[4097] = 0", "not tags[4097] = 0"] {
        assert!(
            selected(&forge, &format!("at \"synthetic\" where {condition}"))?.is_empty(),
            "missing or null does not prove {condition}"
        );
    }
    assert_eq!(
        selected(&forge, "at \"synthetic\" where not tags[0] = 1")?,
        [0],
        "only the Record with a known false comparison matches its negation"
    );
    Ok(())
}

#[test]
fn a_failed_projected_list_leaves_the_direct_cell_budget_available() -> TestResult {
    let bytes = database(4097, None);
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Station select tags[*], tags[4096]")?.compile(&forge)?;
    let run = compiled.run()?;
    let rows = run.rows(0..1);
    let row = rows.first().ok_or("the Station row is selected")?;
    let cells = row.cells().map_err(ToString::to_string)?;
    assert!(
        matches!(cells.first(), Some(Some(Value::Error(ValueError::PathTooWide { limit }))) if *limit == MAX_PATH_VALUES),
        "the projected list reports its existing memory limit"
    );
    assert_eq!(
        cells.get(1),
        Some(&Some(Value::Int32(0))),
        "the failed full-list admission did not spend the direct cell's budget"
    );
    Ok(())
}
