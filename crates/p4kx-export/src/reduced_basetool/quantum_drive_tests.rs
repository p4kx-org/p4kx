// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::quantum_drives;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

#[path = "../../tests/it/catalog_quantum_fixture.rs"]
mod fixture;

use self::fixture::Layout;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn quantum_drives_keep_both_physical_components_and_selected_standard_values() -> TestResult {
    with_rows(Layout::Full, |rows| {
        let [first, second] = rows else {
            return Err("two physical Quantum Drive components".into());
        };
        assert_eq!(
            first
                .pointer("/componentOrdinal/value")
                .and_then(Value::as_u64),
            Some(1),
            "the other component before the drive retained its physical ordinal"
        );
        assert_eq!(
            second
                .pointer("/componentOrdinal/value")
                .and_then(Value::as_u64),
            Some(2)
        );
        for (row, speed) in [(first, 1.0), (second, 2049.0)] {
            let standard = row.pointer("/standard/value").ok_or("standard profile")?;
            for (field, offset) in [
                ("driveSpeed", 0.0),
                ("cooldownTime", 1.0),
                ("spoolUpTime", 2.0),
                ("stageOneAccelRate", 3.0),
                ("stageTwoAccelRate", 4.0),
                ("engageSpeed", 5.0),
                ("calibrationDelayInSeconds", 11.0),
            ] {
                let quantity = standard.get(field).ok_or("selected quantity")?;
                assert_eq!(
                    quantity.pointer("/value/finite").and_then(Value::as_f64),
                    Some(speed + offset),
                    "the original standard profile supplies {field}"
                );
                assert_eq!(
                    quantity
                        .get("containsEncryptedSource")
                        .and_then(Value::as_bool),
                    Some(true),
                    "{field} came from the encrypted DataForge"
                );
                assert!(
                    quantity.pointer("/value/rawBits").is_none(),
                    "the selected quantity carries the numeric projection"
                );
            }
            let fuel = row
                .pointer("/fuelRequirement/value/finite")
                .and_then(Value::as_f64)
                .ok_or("finite authored fuel")?;
            assert!(
                fuel.is_sign_negative() && fuel == 0.0,
                "signed zero remains present"
            );
            for field in ["spline", "heat", "boost", "additionalFields"] {
                assert!(
                    row.get(field).is_none(),
                    "{field} is outside the selected profile"
                );
            }
        }
        Ok(())
    })
}

#[test]
fn missing_standard_remains_absent_and_excluded_spline_issues_stay_outside_scope() -> TestResult {
    for (layout, expected) in [
        (Layout::WithoutStandard, "absent"),
        (Layout::WithoutSpline, "present"),
        (Layout::InvalidSpline, "present"),
    ] {
        with_rows(layout, |rows| {
            let [row] = rows else {
                return Err("one physical Quantum Drive component".into());
            };
            assert_eq!(
                row.pointer("/standard/state").and_then(Value::as_str),
                Some(expected),
                "the standard declaration alone decides its output state"
            );
            if expected == "absent" {
                assert!(row.pointer("/standard/value").is_some_and(Value::is_null));
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn with_rows(layout: Layout, inspect: impl FnOnce(&[Value]) -> TestResult) -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::database(layout).build()).encrypted())
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let table = quantum_drives::collect(&session.parsed(None)?, directory.path(), None)?;
    let value = serde_json::to_value(table)?;
    let rows = value
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("Quantum Drive rows")?;
    inspect(rows)
}
