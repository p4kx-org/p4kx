// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Query selection is complete or fails before stdout and atomic output are written.

use std::error::Error;
use std::fs;

use p4kx_testkit::query_selection_sample;
use serde_json::json;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn late_witnesses_and_legitimate_unknowns_have_distinct_successful_results() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("query.dcb");
    fs::write(&input, query_selection_sample::database().build())?;
    for (condition, rows) in [
        (
            "tags[*] = 4096",
            json!([{"record path":"synthetic/station","id":7}]),
        ),
        ("owner.ready = true", json!([])),
    ] {
        let output = p4kx_testkit::p4kx()?
            .args(["dcb", "query", "--format", "json"])
            .arg(&input)
            .arg(format!("from Station select id where {condition}"))
            .output()?;
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout)?,
            rows,
            "the condition yields the expected complete rows"
        );
    }
    Ok(())
}

#[test]
fn unreadable_selection_fails_query_and_fields_without_a_successful_empty_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("query.dcb");
    fs::write(&input, query_selection_sample::database().build())?;
    for command in ["query", "fields"] {
        let output = p4kx_testkit::p4kx()?
            .args(["dcb", command, "--format", "json"])
            .arg(&input)
            .arg("from Station where ready = true")
            .output()?;
        assert_eq!(output.status.code(), Some(5), "{command}: {output:?}");
        assert!(output.stdout.is_empty(), "{command}: no result was written");
        let error = String::from_utf8(output.stderr)?;
        assert!(
            error.contains("synthetic/station")
                && error.contains("comparison 1")
                && error.contains("bool byte 2"),
            "{command}: {error}"
        );
    }
    Ok(())
}

#[test]
fn a_selection_error_preserves_an_existing_export_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("query.dcb");
    let out = dir.path().join("result.json");
    fs::write(&input, query_selection_sample::database().build())?;
    fs::write(&out, b"complete earlier export")?;
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "query", "--format", "json", "--out"])
        .arg(&out)
        .arg(&input)
        .arg("from Station where ready = true")
        .output()?;
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert!(output.stdout.is_empty(), "no successful export receipt");
    assert_eq!(
        fs::read(&out)?,
        b"complete earlier export",
        "a failed selection preserves the prior file"
    );
    Ok(())
}
