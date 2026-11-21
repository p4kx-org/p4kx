// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{TestResult, original, reason_use};

#[test]
fn actual_cli_failure_mirror_uses_the_same_report_storage_and_original_source_cells() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let original = original(folder.path(), true)?;
    let out = folder.path().join("catalog.json");
    let output = p4kx_testkit::p4kx()?
        .arg("catalog")
        .arg(&original.archive)
        .arg("--out")
        .arg(&out)
        .arg("--diagnostic-partial")
        .arg("--json")
        .env("SOURCE_DATE_EPOCH", "1792000000")
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(1),
        "actual diagnostic CLI exit: {output:?}"
    );
    let summary: Value = serde_json::from_slice(&output.stdout)?;
    let catalog: Value = serde_json::from_slice(&std::fs::read(&out)?)?;
    assert_eq!(
        summary.get("schema_version"),
        Some(&json!(3)),
        "the CLI emits only schema v3"
    );
    let health = summary.get("export_health").ok_or("actual CLI health")?;
    assert_eq!(
        catalog.get("exportHealth"),
        Some(health),
        "the file and summary share one report"
    );
    let row = summary
        .get("failed")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .ok_or("actual legacy-named machine failure")?;
    let details = health
        .get("failures")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .ok_or("actual captured report failure")?;
    mirrors(&catalog, row, details)?;
    assert_eq!(
        text(health, row.get("path").ok_or("actual mirrored path")?)?,
        "libs/foundry/records/scitemmanufacturer/health",
        "the actual original normalized path",
    );
    assert_eq!(
        text(health, row.get("error").ok_or("actual mirrored error")?)?,
        original
            .reason
            .as_deref()
            .ok_or("original canonical decoder reason")?,
        "the actual original formatter result survives the process boundary",
    );
    Ok(())
}

fn text<'a>(health: &'a Value, cell: &Value) -> TestResult<&'a str> {
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "actual retained source state"
    );
    let raw = cell
        .pointer("/value/atomIndex")
        .and_then(Value::as_str)
        .ok_or("actual report-local atom reference")?;
    let index: usize = raw.parse()?;
    assert_eq!(
        index.to_string(),
        raw,
        "the structural index uses canonical decimal text"
    );
    let atom = health
        .pointer("/metadata/texts")
        .and_then(Value::as_array)
        .and_then(|rows| rows.get(index))
        .ok_or("this exact report's shared text atom")?;
    assert_eq!(
        atom.get("state"),
        Some(&json!("present")),
        "the stored aggregate is present"
    );
    atom.get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| "actual stored source text".into())
}

fn closure(catalog: &Value, cell: &Value) -> TestResult {
    let refs = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("actual immediate contribution refs")?;
    assert!(
        !refs.is_empty(),
        "the actual original failure cell has nonvacuous evidence"
    );
    for reference in refs {
        reason_use(
            catalog,
            reference.as_str().ok_or("actual contributing use ID")?,
        )?;
    }
    Ok(())
}

fn mirrors(catalog: &Value, row: &Value, details: &Value) -> TestResult {
    for (mirror, owned) in [
        ("guid", "guid"),
        ("path", "recordPath"),
        ("error", "reason"),
    ] {
        let cell = row.get(mirror).ok_or("actual sourced mirror cell")?;
        assert_eq!(
            Some(cell),
            details.get(owned),
            "the mirror borrows the exact sourced occurrence"
        );
        closure(catalog, cell)?;
    }
    Ok(())
}
