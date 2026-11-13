// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::{TestResult, archive, check_binding, english, profile, rows, run, standard};

#[test]
fn diagnostics_leave_a_contiguous_machine_frame_on_stderr() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = archive(
        dir.path(),
        vec![
            profile(),
            TestEntry::stored(
                "Data/Localization/english/global.ini",
                b"ui_CGSeatGeneral=Map\nbroken line\n".to_vec(),
            ),
        ],
    )?;
    let receipt = dir.path().join("origin.jsonl");
    let output = run(
        &path,
        &[
            "--origin-stderr",
            "--origin-out",
            receipt.to_str().ok_or("receipt")?,
        ],
    )?;
    let durable = std::fs::read(&receipt)?;
    assert!(output.stderr.starts_with(b"note: "));
    assert!(output.stderr.ends_with(&durable));
    let rows = rows(&durable)?;
    check_binding(&rows, &output.stdout)?;
    Ok(())
}

#[test]
fn invalid_profile_leaves_the_old_artifact_and_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let valid = standard(dir.path())?;
    let invalid = dir.path().join("invalid");
    std::fs::create_dir(&invalid)?;
    let invalid = archive(
        &invalid,
        vec![
            TestEntry::stored("Data/Libs/Config/defaultProfile.xml", b"<profile>".to_vec()),
            english(),
        ],
    )?;
    let output = dir.path().join("map.json");
    let receipt = dir.path().join("map.json.p4kx-origin.jsonl");
    run(&valid, &["--out", output.to_str().ok_or("output")?])?;
    let old_artifact = std::fs::read(&output)?;
    let old_origin = std::fs::read(&receipt)?;
    let result = p4kx_testkit::p4kx()?
        .args(["actionmap", "export"])
        .arg(&invalid)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert_eq!(result.status.code(), Some(3));
    assert_eq!(std::fs::read(output)?, old_artifact);
    assert_eq!(std::fs::read(receipt)?, old_origin);
    Ok(())
}

#[test]
fn structured_summary_counts_keep_only_their_actual_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = archive(
        dir.path(),
        vec![
            profile(),
            TestEntry::stored(
                "Data/Localization/english/global.ini",
                b"ui_CGSeatGeneral=Map\nui_CIEject=Label\nui_CIProne=\n".to_vec(),
            )
            .encrypted(),
        ],
    )?;
    let output = dir.path().join("map.csv");
    let report = run(
        &path,
        &[
            "--json",
            "--format",
            "csv",
            "--out",
            output.to_str().ok_or("output")?,
        ],
    )?;
    let report: Value = serde_json::from_slice(&report.stdout)?;
    for (path, expected) in [
        ("/fieldOrigins/~1action_maps/containsEncryptedSource", false),
        ("/fieldOrigins/~1actions/containsEncryptedSource", false),
        (
            "/fieldOrigins/~1unresolved_labels/containsEncryptedSource",
            true,
        ),
        ("/fieldOrigins/~1language/containsEncryptedSource", false),
    ] {
        assert_eq!(
            report.pointer(path).and_then(Value::as_bool),
            Some(expected),
            "{path}"
        );
    }
    check_count_selectors(&report)
}

fn check_count_selectors(report: &Value) -> TestResult {
    let sources = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    assert!(
        sources
            .iter()
            .filter(|source| source.get("scope").and_then(Value::as_str) == Some("payload"))
            .all(|source| source.get("role").and_then(Value::as_str) == Some("selector")),
        "counts are selection-only"
    );
    let names = sources
        .iter()
        .filter(|source| {
            source
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some("globalIniDecodedText")
        })
        .count();
    assert_eq!(names, 3, "found and empty values all affect the count");
    assert_eq!(
        report.get("unresolved_labels").and_then(Value::as_u64),
        Some(2),
        "count unchanged"
    );
    Ok(())
}
