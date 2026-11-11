// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_assembly::character::Chf;
use p4kx_testkit::{TestChf, p4kx};
use serde_json::{Value, json};

use crate::chf_origins::write_mixed_fixture;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn diff_separates_original_old_and_new_values_from_the_encrypted_path_name() -> TestResult {
    let report = encrypted_comparison()?;
    let row = report.pointer("/changes/0").ok_or("comparison row")?;
    assert_eq!(row["old"], "0");
    assert_eq!(row["new"], "1.25");
    assert!(
        row["path"]
            .as_str()
            .ok_or("readable path")?
            .contains("SyntheticOriginParameter")
    );
    assert_eq!(
        report["fieldOrigins"]["/changes/0/path"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        report["fieldOrigins"]["/changes/0/raw_path"]["containsEncryptedSource"],
        Value::Null
    );
    assert_eq!(
        report["fieldOrigins"]["/identical"]["containsEncryptedSource"],
        Value::Null
    );
    for (field, source) in [("/changes/0/old", "f0"), ("/changes/0/new", "f1")] {
        let origin = &report["fieldOrigins"][field];
        assert_eq!(origin["containsEncryptedSource"], Value::Null, "{field}");
        let datum = field_uses(&report, field)?
            .into_iter()
            .next()
            .ok_or("value source")?;
        assert_eq!(
            datum["source"], source,
            "{field} must use its own physical input"
        );
        assert_eq!(
            datum["location"]["field"],
            "/character/materials/0/sub_materials/0/floats/1/value"
        );
    }
    let path = field_uses(&report, "/changes/0/path")?;
    let name_selector: Vec<_> = path
        .into_iter()
        .filter(|datum| {
            datum.pointer("/location/field").and_then(Value::as_str)
                == Some("/character/materials/0/base_material")
        })
        .filter_map(|datum| datum.get("source"))
        .collect();
    assert_eq!(
        name_selector,
        [&json!("f0")],
        "the existing first-wins name table uses the first CHF's material selector"
    );
    Ok(())
}

#[test]
fn an_added_material_records_the_absence_query_against_the_original_list() -> TestResult {
    let dir = tempfile::tempdir()?;
    let before = dir.path().join("before.chf");
    let after = dir.path().join("after.chf");
    let mut old = TestChf::v7();
    old.materials.clear();
    fs::write(&before, old.to_bytes())?;
    fs::write(&after, TestChf::v7().to_bytes())?;
    let command = p4kx()?
        .args(["--json", "chf", "diff"])
        .arg(&before)
        .arg(&after)
        .output()?;
    assert_eq!(command.status.code(), Some(1), "{command:?}");
    let report: Value = serde_json::from_slice(&command.stdout)?;
    assert_eq!(report.pointer("/changes/0/old"), Some(&Value::Null));
    let old = field_uses(&report, "/changes/0/old")?;
    let old = old.first().ok_or("the missing side's source query")?;
    assert_eq!(old["source"], "f0");
    assert_eq!(old["role"], "selector");
    assert_eq!(old["location"]["namespace"], "chfDiffMissingHash");
    assert_eq!(old["location"]["field"], "/character/materials");
    assert_eq!(old["location"]["indices"], json!(["195939070", "0"]));
    Ok(())
}

fn encrypted_comparison() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("fixture.p4k");
    let before = dir.path().join("before.chf");
    let after = dir.path().join("after.chf");
    write_mixed_fixture(&archive, &before, false)?;
    let mut modified = Chf::read(&fs::read(&before)?)?;
    let value = modified
        .character
        .materials
        .first_mut()
        .and_then(|material| material.sub_materials.first_mut())
        .and_then(|material| material.floats.get_mut(1))
        .ok_or("fixture float")?;
    value.value = 1.25;
    fs::write(&after, modified.write()?)?;
    let command = p4kx()?
        .args(["chf", "diff", "--format", "json"])
        .arg(&before)
        .arg(&after)
        .arg("--archive")
        .arg(&archive)
        .output()?;
    assert_eq!(command.status.code(), Some(1), "{command:?}");
    Ok(serde_json::from_slice(&command.stdout)?)
}

fn field_uses<'report>(
    report: &'report Value,
    field: &str,
) -> Result<Vec<&'report Value>, Box<dyn Error>> {
    let refs = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(field))
        .and_then(|field| field.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or("field use references")?;
    let inventory = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("use inventory")?;
    Ok(inventory
        .iter()
        .filter(|datum| datum.get("id").is_some_and(|id| refs.contains(id)))
        .collect())
}
