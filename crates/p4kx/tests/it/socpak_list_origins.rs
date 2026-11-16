// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::{ROOM, SHIP, TestResult, p4kx, ship};

#[test]
fn list_names_retain_the_actual_encrypted_parent_directory() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::deflated(SHIP, ship().build()).encrypted())
        .write_to(&archive)?;
    let output = p4kx()?
        .args(["--json", "--key", &test_key().to_hex(), "socpak", "list"])
        .arg(&archive)
        .args([
            "data/objectcontainers/ships/aegs/SHIP_INTERIOR.socpak",
            ROOM,
        ])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report["containers"],
        json!([SHIP, ROOM]),
        "both output names are the stored spellings"
    );
    assert_container_origins(&report, true)
}

pub(super) fn assert_container_origins(report: &Value, encrypted_parent: bool) -> TestResult {
    let sources = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("container source table")?;
    assert_eq!(sources.len(), 2, "only the two selected container Entries");
    let owner = |path: &str| {
        sources
            .iter()
            .find(|source| source.pointer("/path/value").and_then(Value::as_str) == Some(path))
            .ok_or("actual selected container Entry")
    };
    let outer = owner(SHIP)?;
    let inner = owner(ROOM)?;
    let encryption = if encrypted_parent {
        "encrypted"
    } else {
        "unencrypted"
    };
    assert_eq!(outer["storedEncryption"], encryption, "outer payload state");
    assert_eq!(
        inner["storedEncryption"], "unknown",
        "the standard ZIP declaration does not prove payload encryption"
    );
    assert_eq!(
        inner["ancestors"],
        json!([outer["id"]]),
        "the exact outer Entry owns the inner directory"
    );
    let payload = (&outer["id"], "payload");
    let inner_route = [(&outer["id"], "selector"), (&inner["id"], "payload")];
    assert_name_origin(report, "/archive", false, &[])?;
    assert_name_origin(report, "/containers/0", false, &[payload])?;
    assert_name_origin(report, "/containers/1", encrypted_parent, &inner_route)?;
    assert_name_origin(
        report,
        "/containers",
        encrypted_parent,
        &[payload, inner_route[0], inner_route[1]],
    )
}

fn assert_name_origin(
    report: &Value,
    path: &str,
    encrypted: bool,
    expected: &[(&Value, &str)],
) -> TestResult {
    let origin = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or("expected named origin")?;
    assert_eq!(
        origin["sourceSetComplete"], true,
        "complete proof for {path}"
    );
    assert_eq!(
        origin["containsEncryptedSource"], encrypted,
        "directory value encryption for {path}"
    );
    let references = origin["sourceUsesRefs"].as_array().ok_or("source uses")?;
    assert_eq!(references.len(), expected.len(), "exact uses for {path}");
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("use table")?;
    for &(source, role) in expected {
        let usage = uses
            .iter()
            .find(|usage| {
                references.contains(&usage["id"])
                    && &usage["source"] == source
                    && usage["role"].as_str() == Some(role)
            })
            .ok_or_else(|| format!("missing {role} contribution from {source} for {path}"))?;
        assert_eq!(usage["scope"], "archiveDirectoryMetadata", "name scope");
        assert_eq!(
            usage["location"],
            json!({"type": "metadata", "field": "path"}),
            "actual directory name field"
        );
    }
    Ok(())
}
