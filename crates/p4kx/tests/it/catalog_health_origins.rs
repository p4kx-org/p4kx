// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, Record, Value as Raw};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn catalog_machine_summary_retains_direct_health_origins_on_success_and_diagnostic_output()
-> TestResult {
    for broken in [false, true] {
        let folder = tempfile::tempdir()?;
        let original = original(folder.path(), broken)?;
        let out = folder.path().join("catalog.json");
        let mut command = p4kx_testkit::p4kx()?;
        command
            .arg("catalog")
            .arg(&original.archive)
            .arg("--out")
            .arg(&out)
            .arg("--json")
            .env("SOURCE_DATE_EPOCH", "1792000000");
        if broken {
            command.arg("--diagnostic-partial");
        }
        let output = command.output()?;
        assert_eq!(
            output.status.code(),
            Some(i32::from(broken)),
            "actual complete/diagnostic CLI exit contract: {output:?}"
        );
        let summary: Value = serde_json::from_slice(&output.stdout)?;
        let catalog: Value = serde_json::from_slice(&std::fs::read(&out)?)?;
        assert_eq!(
            summary.get("schema_version"),
            Some(&json!(3)),
            "the only current writer schema is v3"
        );
        assert_eq!(
            catalog.get("schemaVersion"),
            Some(&json!(3)),
            "the file and machine summary share the v3 contract"
        );
        let health = summary
            .get("export_health")
            .ok_or("actual machine health")?;
        assert_eq!(
            catalog.get("exportHealth"),
            Some(health),
            "both outputs retain the same measured report"
        );
        if broken {
            failed(
                &summary,
                &catalog,
                original
                    .reason
                    .as_deref()
                    .ok_or("original derived failure reason")?,
            )?;
        } else {
            example(health)?;
        }
    }
    Ok(())
}
struct OriginalFixture {
    archive: PathBuf,
    reason: Option<String>,
}

fn original(folder: &Path, broken: bool) -> TestResult<OriginalFixture> {
    let definition = TestStruct::new("SCItemManufacturer").field("Code", TestType::String);
    let definition = if broken {
        definition.declared_size(5)
    } else {
        definition
    };
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(definition)
        .record(
            TestRecord::new(
                0x5741,
                "SCItemManufacturer",
                "SCItemManufacturer.Health",
                "libs/foundry/records/scitemmanufacturer/health.xml",
            )
            .value("Code", TestValue::text("")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("original CLI subject")?;
    assert_eq!(record.index(), 0, "original physical owner");
    let reason = original_outcome(&record, broken)?;
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes))
        .write_to(&path)?;
    Ok(OriginalFixture {
        archive: path,
        reason,
    })
}
fn original_outcome(record: &Record<'_>, broken: bool) -> TestResult<Option<String>> {
    if broken {
        let error = record
            .tree()
            .err()
            .ok_or("actual original layout failure")?;
        assert!(
            matches!(
                &error,
                DataForgeError::StructLayout {
                    declared: 5,
                    fields: 4,
                    ..
                }
            ),
            "the original decoder proves the intended CLI source failure"
        );
        Ok(Some(error.to_string()))
    } else {
        assert_eq!(
            record.tree()?.root().field("Code"),
            Some(&Raw::String("")),
            "the successful example comes from a real retained empty original field"
        );
        Ok(None)
    }
}

fn direct<'a>(row: &'a Value, field: &str) -> TestResult<&'a Value> {
    let cell = row.get(field).ok_or("actual machine identity")?;
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "the CLI {field} has its own direct carrier"
    );
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the original CLI DCB is plain"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "the actual source identity is completely observed"
    );
    assert!(
        !cell
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("actual machine source references")?
            .is_empty(),
        "copied original identities keep their actual uses"
    );
    Ok(cell)
}
fn failed(summary: &Value, catalog: &Value, reason: &str) -> TestResult {
    let row = summary
        .pointer("/export_health/failedRecords/0")
        .ok_or("actual failed health identity")?;
    direct(row, "guid")?;
    let mirror = summary
        .pointer("/failed/0")
        .ok_or("actual CLI failure mirror")?;
    assert_eq!(
        direct(mirror, "guid")?.get("value"),
        row.pointer("/guid/value"),
        "the CLI mirror uses the captured original failure rather than reparsing a DTO"
    );
    assert_eq!(
        direct(mirror, "path")?.get("value"),
        row.pointer("/recordPath/value"),
        "the CLI mirror preserves the exact same original path"
    );
    derived_reason(catalog, mirror, reason)
}
fn example(health: &Value) -> TestResult {
    let diagnostic = health
        .get("diagnostics")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("code") == Some(&json!("emptyValue")))
        })
        .ok_or("actual informational empty-field diagnostic")?;
    let example = diagnostic
        .pointer("/examples/0")
        .ok_or("actual successful summary example")?;
    direct(example, "recordGuid")?;
    direct(example, "recordPath")?;
    Ok(())
}

fn reason_carrier(row: &Value) -> TestResult<&Value> {
    let cell = row.get("error").ok_or("actual structured failure reason")?;
    let names: BTreeSet<_> = cell
        .as_object()
        .ok_or("direct derived reason carrier")?
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "state",
            "value",
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete"
        ]),
        "the derived error text follows the direct five-field source contract"
    );
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "the actual reason is a present derived value"
    );
    Ok(cell)
}

pub(super) fn retained_health_text<'a>(health: &'a Value, cell: &Value) -> TestResult<&'a str> {
    let reference = cell
        .get("value")
        .and_then(Value::as_object)
        .ok_or("health text reference")?;
    assert_eq!(
        reference.len(),
        1,
        "the reference contains only the report-local atom index"
    );
    let raw = reference
        .get("atomIndex")
        .and_then(Value::as_str)
        .ok_or("health text index")?;
    let index: usize = raw.parse()?;
    assert_eq!(
        index.to_string(),
        raw,
        "the atom index is canonical decimal"
    );
    let atom = health
        .pointer("/metadata/texts")
        .and_then(Value::as_array)
        .and_then(|texts| texts.get(index))
        .ok_or("retained text in this health report")?;
    assert_eq!(
        atom.get("state"),
        Some(&json!("present")),
        "the referenced text is present"
    );
    atom.get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| "retained health text".into())
}

fn derived_reason(catalog: &Value, row: &Value, expected: &str) -> TestResult {
    let cell = reason_carrier(row)?;
    let health = catalog.get("exportHealth").ok_or("owning health report")?;
    assert_eq!(
        retained_health_text(health, cell)?,
        expected,
        "the actual original decoder reason is preserved verbatim"
    );
    let references = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("reason contribution references")?;
    let complete = cell
        .get("sourceSetComplete")
        .and_then(Value::as_bool)
        .ok_or("reason capture completeness")?;
    let converted = reason_inputs(catalog, references)?;
    match (cell.get("containsEncryptedSource"), complete) {
        (Some(Value::Bool(false)), true) => {
            assert!(!references.is_empty() && converted,
                "a completely captured derived plain reason uses actual original formatting inputs");
        }
        (Some(Value::Null), false) => {
            assert!(!references.is_empty() || attempted_original(catalog)?,
                "an unclosed original formatting capture keeps its known prefix or actual DCB attempt");
        }
        _ => return Err("a plain original derived reason cannot invent encryption, completeness or independence".into()),
    }
    Ok(())
}

fn reason_inputs(catalog: &Value, references: &[Value]) -> TestResult<bool> {
    let mut seen = BTreeSet::new();
    let mut converted = false;
    for reference in references {
        let id = reference.as_str().ok_or("actual reason use reference")?;
        assert!(
            seen.insert(id),
            "the reason retains distinct actual contributions"
        );
        let used = reason_use(catalog, id)?;
        converted |= used.get("role") == Some(&json!("conversion"));
    }
    Ok(converted)
}

fn reason_use<'a>(catalog: &'a Value, id: &str) -> TestResult<&'a Value> {
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("actual catalog use inventory")?;
    let matching: Vec<_> = inventory
        .iter()
        .filter(|used| used.get("id").and_then(Value::as_str) == Some(id))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "each reason reference resolves in the actual emitted build inventory"
    );
    let used = matching
        .first()
        .copied()
        .ok_or("actual reason contribution")?;
    assert_eq!(
        used.get("scope"),
        Some(&json!("payload")),
        "struct layout formatting consumes DCB payload metadata"
    );
    let role = used
        .get("role")
        .and_then(Value::as_str)
        .ok_or("actual reason role")?;
    assert!(
        matches!(role, "payload" | "selector" | "conversion"),
        "attempts are not contributing reason references"
    );
    assert!(
        original_dcb(catalog, used)?,
        "the original DCB supplies the formatting input"
    );
    Ok(used)
}

fn attempted_original(catalog: &Value) -> TestResult<bool> {
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("actual attempted source inventory")?;
    for used in inventory {
        if used.get("role") == Some(&json!("attempted"))
            && used.get("scope") == Some(&json!("payload"))
            && original_dcb(catalog, used)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn original_dcb(catalog: &Value, used: &Value) -> TestResult<bool> {
    let sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("actual physical source inventory")?;
    let id = used.get("source").ok_or("original reason source")?;
    let matching: Vec<_> = sources
        .iter()
        .filter(|source| source.get("id") == Some(id))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "a formatting source resolves to one actual physical input"
    );
    let source = matching.first().ok_or("original formatting input")?;
    Ok(source.get("kind") == Some(&json!("archiveEntry"))
        && source.pointer("/path/value") == Some(&json!("Data\\Game2.dcb")))
}

#[path = "catalog_owned_failure_views.rs"]
mod owned_failure_views;
