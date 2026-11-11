// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Catalog warning output separates technical diagnostics from authored values.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{DataForge, Value as ForgeValue};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use super::catalog_health_origins::retained_health_text;

type TestResult = Result<(), Box<dyn Error>>;

const SOURCE_VALUE: &str = "synthetic_private_mission_value_341";

#[test]
fn unknown_vocabulary_remains_sourced_in_the_file_and_out_of_warning_messages() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let destination = dir.path().join("catalog.json");
    let bytes = source();
    assert_original(&bytes)?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", bytes).encrypted())
        .write_to(&path)?;
    let normal = run(&path, &destination, &[])?;
    assert_eq!(
        normal.status.code(),
        Some(1),
        "a real source warning blocks normal publication"
    );
    assert!(!destination.exists(), "normal mode creates no destination");
    let diagnostic = run(&path, &destination, &["--diagnostic-partial"])?;
    assert_eq!(
        diagnostic.status.code(),
        Some(1),
        "written diagnostic remains incomplete"
    );
    let stderr = String::from_utf8(diagnostic.stderr)?;
    let stdout = String::from_utf8(diagnostic.stdout)?;
    let summary: Value = serde_json::from_str(&stdout)?;
    let warnings = summary.get("warnings").ok_or("CLI warnings")?;
    assert!(
        !stderr.contains(SOURCE_VALUE) && !stdout.contains(SOURCE_VALUE),
        "warning surfaces only carry technical classifications and counts"
    );
    assert_eq!(
        warnings,
        &json!([{ "kind": "unknown_template_values", "count": 1, "details": ["timer end result (1)"] }]),
        "stable CLI warning discriminator and exact category count"
    );
    let catalog: Value = serde_json::from_slice(&fs::read(&destination)?)?;
    assert_artifact(&catalog)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_artifact(catalog: &Value) -> TestResult {
    let health = catalog.get("exportHealth").ok_or("artifact health")?;
    assert_eq!(
        health.get("status"),
        Some(&json!("incomplete")),
        "source warning affects artifact health"
    );
    assert!(
        !health.to_string().contains(SOURCE_VALUE),
        "artifact messages omit raw game values"
    );
    let diagnostics = health
        .get("diagnostics")
        .and_then(Value::as_array)
        .ok_or("artifact diagnostics")?;
    let warning = diagnostics
        .iter()
        .find(|item| {
            item.get("kind") == Some(&json!("missionBrokerEntries"))
                && item.get("warningCode") == Some(&json!("unknown_template_values"))
        })
        .ok_or("stable artifact warning for the source Broker kind")?;
    assert_eq!(
        warning.get("count"),
        Some(&json!(1)),
        "original warning occurrence count"
    );
    let source = original_source(catalog)?;
    let templates = diagnostics
        .iter()
        .find(|item| {
            item.get("kind") == Some(&json!("contractTemplates"))
                && item.get("warningCode") == Some(&json!("unknown_template_values"))
        })
        .ok_or("shared warning family status for Templates")?;
    assert_eq!(
        templates.get("examples"),
        Some(&json!([])),
        "the Broker field does not manufacture a Template occurrence"
    );
    warning_example(catalog, health, warning, source)
}

fn original_source(catalog: &Value) -> Result<&Value, Box<dyn Error>> {
    let values = catalog
        .pointer("/definitions/missionBrokerEntries/rows/0/source/values")
        .and_then(Value::as_array)
        .ok_or("physical Broker source arena")?;
    let source = values
        .iter()
        .filter_map(|node| node.get("value"))
        .find(|cell| {
            cell.pointer("/value/value/sourceText")
                .and_then(Value::as_str)
                == Some(SOURCE_VALUE)
        })
        .ok_or("authored vocabulary value remains in its source arena")?;
    assert_eq!(
        source.get("containsEncryptedSource"),
        Some(&json!(true)),
        "original datum keeps its direct encrypted provenance"
    );
    assert_eq!(
        source.get("sourceSetComplete"),
        Some(&json!(true)),
        "original source use is known"
    );
    assert!(
        source
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|uses| !uses.is_empty()),
        "the encrypted datum retains its exact source-use references"
    );
    Ok(source)
}

fn warning_example(catalog: &Value, health: &Value, warning: &Value, source: &Value) -> TestResult {
    let examples = warning
        .get("examples")
        .and_then(Value::as_array)
        .ok_or("warning examples")?;
    let [example] = examples.as_slice() else {
        return Err("exactly one source-bound Broker warning sample".into());
    };
    let field = example.get("field").ok_or("warning field carrier")?;
    assert_eq!(
        retained_health_text(health, field)?,
        "missionDeadline.missionResultAfterTimerEnd",
        "only the technical field path is displayed"
    );
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(true)),
        "direct warning field origin"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(true)),
        "exact source fact is known"
    );
    let original = contributions(catalog, source)?
        .into_iter()
        .find(|used| {
            used.get("role") == Some(&json!("payload"))
                && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                && used.pointer("/location/field") == Some(&json!("$value"))
        })
        .ok_or("original authored payload contribution")?;
    let matches = contributions(catalog, field)?.into_iter().any(|used| {
        used.get("source") == original.get("source")
            && used.get("scope") == Some(&json!("payload"))
            && used.get("location") == original.get("location")
            && used.get("role") == Some(&json!("selector"))
    });
    assert!(
        matches,
        "warning selects the exact original value at the same physical source location"
    );
    Ok(())
}

fn contributions<'catalog>(
    catalog: &'catalog Value,
    datum: &Value,
) -> Result<Vec<&'catalog Value>, Box<dyn Error>> {
    let references = datum
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("datum use references")?;
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("original uses inventory")?;
    references
        .iter()
        .map(|id| {
            inventory
                .iter()
                .find(|used| used.get("id") == Some(id))
                .ok_or_else(|| "referenced original use".into())
        })
        .collect()
}

fn source() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("BrokerDeadline").field("missionResultAfterTimerEnd", TestType::String),
        )
        .structure(TestStruct::new("MissionBrokerEntry").field(
            "missionDeadline",
            TestType::Class("BrokerDeadline".to_owned()),
        ))
        .record(
            TestRecord::new(
                0x1049,
                "MissionBrokerEntry",
                "MissionBrokerEntry.Privacy",
                "libs/foundry/records/missionbroker/privacy.xml",
            )
            .value(
                "missionDeadline",
                TestValue::Struct(
                    TestInstance::new("BrokerDeadline")
                        .value("missionResultAfterTimerEnd", TestValue::text(SOURCE_VALUE)),
                ),
            ),
        )
        .build()
}

fn assert_original(bytes: &[u8]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let record = forge.record(0).ok_or("original Broker record")?;
    assert_eq!(
        record.type_name()?,
        "MissionBrokerEntry",
        "real selected Record type"
    );
    let tree = record.tree()?;
    let Some(ForgeValue::Struct(deadline)) = tree.field("missionDeadline") else {
        return Err("original deadline class".into());
    };
    assert!(
        matches!(deadline.field("missionResultAfterTimerEnd"),
            Some(ForgeValue::String(text)) if *text == SOURCE_VALUE),
        "raw test value is physically in the source"
    );
    Ok(())
}

fn run(archive: &Path, out: &Path, args: &[&str]) -> Result<std::process::Output, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?
        .arg("catalog")
        .arg(archive)
        .arg("--out")
        .arg(out)
        .arg("--json")
        .args(args)
        .output()?)
}
