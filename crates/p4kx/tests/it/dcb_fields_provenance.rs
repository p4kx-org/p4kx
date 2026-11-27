// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::{TestResult, database, stdout};

#[test]
fn json_binds_each_displayed_metric_to_encrypted_original_uses() -> TestResult {
    let text = stdout(&["--json", "--path", "crew"])?;
    let result: Value = serde_json::from_str(&text)?;
    assert_eq!(
        result.get("field_count"),
        Some(&json!(1)),
        "only one field is displayed"
    );
    assert_eq!(
        result.pointer("/fields/0/path"),
        Some(&json!("crew")),
        "the proof follows display order"
    );
    for path in [
        "/records",
        "/unreadable_records",
        "/unreadable_values",
        "/field_count",
        "/fields/0/path",
        "/fields/0/max",
        "/fields/0/kinds/0/kind",
        "/fields/0/kinds/0/count",
    ] {
        let origin = origin(&result, path)?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{path} has a proved encrypted source"
        );
        assert_uses_resolve(&result, origin)?;
    }
    check_selection_refs(&result)?;
    assert!(
        result
            .get("fieldOrigins")
            .and_then(|origins| origins.get("/fields/1/min"))
            .is_none(),
        "the excluded fields have no output addresses"
    );
    assert_eq!(
        result.pointer("/sourceProof/selectionMode"),
        Some(&json!("allRecords")),
        "standalone all-Record selection carries one actual shared basis"
    );
    Ok(())
}

#[test]
fn clear_complete_selection_and_empty_filter_retain_counter_proof() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()))
        .write_to(&input)?;
    let (result, stderr) = run(&input, &["--json", "--path", "unmatched-field"], 0)?;
    assert!(
        stderr.is_empty(),
        "the filter admits zero fields without error: {stderr}"
    );
    assert_eq!(
        result.get("fields"),
        Some(&json!([])),
        "the filter emits no rows"
    );
    assert_eq!(
        result.get("field_count"),
        Some(&json!(0)),
        "zero admitted rows"
    );
    let count = origin(&result, "/field_count")?;
    assert_eq!(
        count.get("containsEncryptedSource"),
        Some(&json!(false)),
        "all actual origins are proven clear"
    );
    assert_eq!(
        count.get("sourceSetComplete"),
        Some(&json!(true)),
        "the complete shape proves the count after filtering"
    );
    assert_uses_resolve(&result, count)?;
    Ok(())
}

#[test]
fn fully_observed_header_query_marks_a_filtered_clear_metric() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()))
        .write_to(&input)?;
    let (result, stderr) = run(
        &input,
        &[
            r#"from Ship at "libs/ships/t*""#,
            "--json",
            "--path",
            "crew",
        ],
        0,
    )?;
    assert!(
        stderr.is_empty(),
        "the exact selected Record is readable: {stderr}"
    );
    assert_eq!(
        result.pointer("/sourceProof/selection/sourceSetComplete"),
        Some(&json!(true)),
        "the complete original headers prove the type and path selection"
    );
    assert_eq!(
        result.pointer("/fields/0/max"),
        Some(&json!(4)),
        "the selected Record supplies the displayed numeric value"
    );
    let proof = origin(&result, "/fields/0/max")?;
    assert_eq!(
        proof.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the complete original instance and selection are both proved clear"
    );
    assert_uses_resolve(&result, proof)?;
    Ok(())
}

#[test]
fn raw_dataforge_partial_read_preserves_unknown_lineage_and_failed_root_uses() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("Game2.dcb");
    let source = DataForgeBuilder::new()
        .structure(TestStruct::new("Owner").field("number", TestType::Int32))
        .record(
            TestRecord::new(1, "Owner", "Good", "libs/good.xml")
                .value("number", TestValue::Int32(9)),
        )
        .record(
            TestRecord::new(2, "Owner", "Unreadable", "libs/unreadable.xml")
                .with_instance_index(u16::MAX),
        )
        .build();
    std::fs::write(&input, source)?;
    let (result, stderr) = run(&input, &["--format", "json"], 1)?;
    assert!(
        stderr.contains("could not be read"),
        "existing partial warning: {stderr}"
    );
    assert_eq!(
        result.get("unreadable_records"),
        Some(&json!(1)),
        "the original failed root is counted"
    );
    let proof = origin(&result, "/fields/0/max")?;
    assert_eq!(
        proof.get("containsEncryptedSource"),
        Some(&Value::Null),
        "a standalone decoded file has unknown original P4K lineage"
    );
    assert_eq!(
        proof.get("sourceSetComplete"),
        Some(&json!(false)),
        "the missing root can hide additional numeric observations"
    );
    let [attempt] = result
        .pointer("/sourceProof/rootAttempts")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("the failed root uses are retained")?
    else {
        return Err("one original root must fail".into());
    };
    let site = use_site(&result, attempt)?;
    assert_eq!(
        site.get("role"),
        Some(&json!("attempted")),
        "the actual failed site stays separate"
    );
    assert!(
        !contributor_refs(&result, proof)?.contains(attempt),
        "the attempted root is not a complete contributing operand for the statistic"
    );
    Ok(())
}

fn origin<'result>(
    result: &'result Value,
    pointer: &str,
) -> Result<&'result Value, Box<dyn Error>> {
    Ok(result
        .get("fieldOrigins")
        .and_then(|origins| origins.get(pointer))
        .ok_or("missing statistic origin")?)
}

fn use_site<'result>(result: &'result Value, id: &Value) -> Result<&'result Value, Box<dyn Error>> {
    let uses = result
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("missing original use inventory")?;
    Ok(uses
        .iter()
        .find(|site| site.get("id") == Some(id))
        .ok_or("the source-use reference must resolve")?)
}

fn assert_uses_resolve(result: &Value, origin: &Value) -> TestResult {
    let refs = contributor_refs(result, origin)?;
    assert!(
        refs.iter().any(|id| use_site(result, id).is_ok()),
        "each derived origin carries at least one actual completed use"
    );
    for id in refs {
        let site = use_site(result, id)?;
        assert_ne!(
            site.get("role"),
            Some(&json!("attempted")),
            "an attempt does not affect the field encryption marker"
        );
    }
    Ok(())
}

fn contributor_refs<'a>(result: &Value, origin: &'a Value) -> Result<&'a [Value], Box<dyn Error>> {
    let refs = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("missing contributor references")?;
    let mut previous = None;
    for reference in refs {
        let text = reference.as_str().ok_or("invalid original use reference")?;
        if previous.is_some_and(|before| before >= text) {
            return Err("unordered or duplicate original use reference".into());
        }
        use_site(result, reference)?;
        previous = Some(text);
    }
    Ok(refs)
}

fn check_selection_refs(result: &Value) -> TestResult {
    let selection = result
        .pointer("/sourceProof/selection")
        .ok_or("selection")?;
    let selected = contributor_refs(result, selection)?;
    for pointer in ["/records", "/unreadable_records"] {
        assert_eq!(
            contributor_refs(result, origin(result, pointer)?)?,
            selected,
            "{pointer} retains the exact selection"
        );
    }
    for pointer in ["/unreadable_values", "/field_count"] {
        let contributors = contributor_refs(result, origin(result, pointer)?)?;
        assert!(
            selected.iter().all(|id| contributors.contains(id)),
            "{pointer} retains the full actual selection"
        );
    }
    let path = contributor_refs(result, origin(result, "/fields/0/path")?)?;
    assert!(
        selected.iter().all(|id| path.contains(id)),
        "the path keeps its selection"
    );
    let maximum = contributor_refs(result, origin(result, "/fields/0/max")?)?;
    assert!(
        selected.iter().all(|id| maximum.contains(id)) && maximum.len() > selected.len(),
        "the maximum retains its selection and the additional original operand"
    );
    Ok(())
}

#[test]
fn contributor_refs_refuse_unknown_uses() -> TestResult {
    let text = stdout(&["--json", "--path", "crew"])?;
    let result: Value = serde_json::from_str(&text)?;
    let mut forged = origin(&result, "/fields/0/path")?.clone();
    forged["sourceUsesRefs"] = json!(["unknown-foreign-use"]);
    assert!(
        contributor_refs(&result, &forged).is_err(),
        "a foreign reference cannot contribute"
    );
    Ok(())
}

fn run(input: &Path, args: &[&str], expected: i32) -> Result<(Value, String), Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "fields"])
        .arg(input)
        .args(args)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(expected),
        "dcb fields exit code: {output:?}"
    );
    Ok((
        serde_json::from_slice(&output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}
