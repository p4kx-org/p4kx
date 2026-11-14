// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{ArchiveBuilder, TestEntry, cargo_grid_facility_database};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn catalog_cli_writes_one_row_per_port_with_original_cargo_grid_and_storage_slots() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let output_file = folder.path().join("catalog.json");
    ArchiveBuilder::new()
        .entry(
            TestEntry::stored("Data\\Game2.dcb", cargo_grid_facility_database().build())
                .encrypted(),
        )
        .write_to(&archive)?;
    let output = p4kx_testkit::p4kx()?
        .arg("catalog")
        .arg(&archive)
        .arg("--out")
        .arg(&output_file)
        .arg("--json")
        .env("SOURCE_DATE_EPOCH", "1792000000")
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "authored nonfinite bits are representable in ordinary v3 publication"
    );
    let summary: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        summary.get("schema_version"),
        Some(&json!(3)),
        "the machine summary reports the schema actually written"
    );
    let catalog: Value = serde_json::from_slice(&std::fs::read(&output_file)?)?;
    assert_eq!(
        catalog.get("schemaVersion"),
        Some(&json!(3)),
        "the grouped port contract belongs to v3"
    );
    assert_eq!(
        catalog
            .pointer("/configurations/facilities/rows/0/crateGenPercentageOnDestroy/value/rawBits"),
        Some(&json!("7fc00042")),
        "the ordinary CLI artifact keeps the original nonfinite bits"
    );
    let rows = catalog
        .pointer("/placements/facilityInstances/rows")
        .and_then(Value::as_array)
        .ok_or("facility placements in the actual CLI artifact")?;
    let [left, right] = rows.as_slice() else {
        return Err("one row per actual port".into());
    };
    check_port(left, 0);
    check_port(right, 1);
    Ok(())
}

fn check_port(placement: &Value, port: u32) {
    assert_eq!(
        placement.pointer("/source/position/value/portOrdinals"),
        Some(&json!([port])),
        "physical port ordinal in the CLI artifact"
    );
    for (index, original, kind) in [(0, 1, "inventory"), (1, 2, "facility")] {
        let link = format!("/configurations/{index}");
        assert_eq!(
            placement.pointer(&format!("{link}/kind")),
            Some(&json!(kind)),
            "links follow the order of the source component slots"
        );
        let slot = format!("{link}/owner/componentOrdinal");
        assert_eq!(
            placement.pointer(&format!("{slot}/value")),
            Some(&json!(original)),
            "original ordinal at placement link {index}"
        );
        assert_eq!(
            placement.pointer(&format!("{slot}/containsEncryptedSource")),
            Some(&json!(true)),
            "encrypted origin at placement link {index}"
        );
    }
    assert!(
        placement.get("configuration").is_none(),
        "each CLI placement uses the same array contract"
    );
}
