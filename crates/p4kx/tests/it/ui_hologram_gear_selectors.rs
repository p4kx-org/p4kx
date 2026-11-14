// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_testkit::{LANDING_GEAR_SHIP, TestEntry, landing_gear_entries, test_key};
use serde_json::Value;

use super::super::hologram_for;
use super::{TestResult, receipt, skip, source_uses, write_archive};

fn fixture(parent_model: bool) -> Vec<TestEntry> {
    landing_gear_entries()
        .into_iter()
        .filter(|entry| {
            parent_model
                || !(entry.path().ends_with("ship.cgf") || entry.path().ends_with("ship.cgfm"))
        })
        .map(|entry| {
            if entry.path().ends_with("Game2.dcb") {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect()
}

#[test]
fn only_evaluated_landing_gear_declarations_reach_the_rendered_artifact() -> TestResult {
    let name = "only_evaluated_landing_gear_declarations_reach_the_rendered_artifact";
    let key = test_key().to_hex();
    for parent_model in [true, false] {
        let dir = tempfile::tempdir()?;
        let archive = write_archive(dir.path(), fixture(parent_model))?;
        let output = dir.path().join("ship.png");
        let run = hologram_for(
            &archive,
            &output,
            LANDING_GEAR_SHIP,
            &["--size", "96", "--key", &key],
            true,
        )?;
        if skip(name, &run) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&output, &report)?;
        assert!(
            !source_uses(&rows, &report, "gear.cgfm").is_empty(),
            "the gear is drawn"
        );
        assert_selectors(&rows, &report, parent_model);
    }
    Ok(())
}

fn assert_selectors(rows: &[Value], report: &Value, parent_model: bool) {
    let uses = source_uses(rows, report, "Game2.dcb");
    let actual: BTreeSet<_> = uses
        .iter()
        .filter_map(|row| {
            let field = row.pointer("/location/field")?.as_str()?;
            let record = row.pointer("/location/recordIndex")?.as_u64()?;
            if field.starts_with("gears[") || field.ends_with(".landingSystem") {
                assert_eq!(
                    row.get("role").and_then(Value::as_str),
                    Some("selector"),
                    "the authored {field} directs selection"
                );
                Some((record, field))
            } else {
                None
            }
        })
        .collect();
    let mut expected = BTreeSet::from([
        (0, "Components[2].landingSystem"),
        (1, "Components[1].landingSystem"),
        (3, "gears[0].geometry.path"),
        (3, "gears[1].geometry.path"),
        (4, "gears[0].geometry.path"),
        (4, "gears[0].bone"),
    ]);
    if parent_model {
        expected.extend([(3, "gears[0].bone"), (3, "gears[1].bone")]);
    }
    assert_eq!(
        actual, expected,
        "exact original Entity and gear slots, including the evaluated missing helper"
    );
}
