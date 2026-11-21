// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn info_json_uses_the_shared_cached_layout_without_changing_existing_counts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    write_build_manifest(directory.path(), "branch", "1.2.3", "456", "build-id")?;
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let info = read_info(&path)?;
    let expected: Value = serde_json::from_str(include_str!(
        "../../../p4kx-testkit/fixtures/archive-layout-v2-empty.json"
    ))?;
    assert_eq!(
        info.get("metadata"),
        Some(&expected),
        "the shared layout retains the independent V2 fixture declarations"
    );
    assert_eq!(
        info.pointer("/fieldOrigins/~1metadata~1v2~1sectorSize/containsEncryptedSource"),
        Some(&json!(false)),
        "CLI addresses each layout field"
    );
    assert_eq!(
        info.pointer("/fieldOrigins/~1metadata~1v2~1sectorSize/sourceSetComplete"),
        Some(&json!(true)),
        "CLI exposes contributor completeness"
    );
    assert!(
        info.pointer("/provenance/uses/0").is_some(),
        "CLI carries the referenced inventory"
    );
    let origins = info.get("fieldOrigins").ok_or("field origins")?;
    check_directory_counts(&info, origins)?;
    check_manifest_origins(&info, origins)
}

fn check_directory_counts(info: &Value, origins: &Value) -> TestResult {
    for (key, count) in [
        ("entry_count", 1),
        ("encrypted_count", 0),
        ("compressed_size", 0),
        ("size", 0),
    ] {
        assert_eq!(
            info.get(key).and_then(Value::as_u64),
            Some(count),
            "the one empty, plain Entry determines {key}"
        );
        let marker = origins.get(format!("/{key}")).ok_or("aggregate origin")?;
        assert_eq!(
            marker
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(false),
            "admitted directory metadata supplies {key}"
        );
        assert_eq!(
            marker.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "the complete original admitted inventory supplies {key}"
        );
    }
    assert_eq!(
        info.get("issue_count").and_then(Value::as_u64),
        Some(0),
        "the empty plain Entry has no eager directory defect"
    );
    assert_eq!(
        info.pointer("/fieldOrigins/~1issue_count"),
        Some(&json!({
            "containsEncryptedSource": null,
            "sourceUsesRefs": [],
            "sourceSetComplete": false,
        })),
        "CLI explicitly marks the missing complete directory-validation receipt as unknown"
    );
    Ok(())
}

fn check_manifest_origins(info: &Value, origins: &Value) -> TestResult {
    assert_eq!(
        info.pointer("/build/branch").and_then(Value::as_str),
        Some("branch"),
        "the summary retains the original manifest Branch"
    );
    let build_marker = origins.get("/build/branch").ok_or("branch origin")?;
    assert_eq!(
        build_marker
            .get("sourceSetComplete")
            .and_then(Value::as_bool),
        Some(true),
        "the original manifest field and parse selectors are available"
    );
    assert_eq!(
        build_marker.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the external manifest has unknown P4K lineage"
    );
    let references = build_marker
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("branch uses")?;
    let inventory = info
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("inventory uses")?;
    assert!(
        inventory.iter().any(|site| {
            site.get("id").is_some_and(|id| references.contains(id))
                && site.get("scope").and_then(Value::as_str) == Some("payload")
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("build-manifest")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("Data.Branch")
        }),
        "the Branch addresses its own manifest field"
    );
    let patch_marker = origins.get("/build/public_patch").ok_or("patch origin")?;
    assert_eq!(
        patch_marker
            .get("sourceSetComplete")
            .and_then(Value::as_bool),
        Some(false),
        "the manifest lookup key does not establish the launcher log contribution"
    );
    Ok(())
}

#[test]
fn info_json_saturates_the_sum_of_declared_sizes_without_reading_the_payload() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("large.bin", b"x").with_uncompressed_size(u64::MAX - 1))
        .entry(TestEntry::stored("small.bin", b"yz"))
        .write_to(&path)?;

    let info = read_info(&path)?;
    assert_eq!(
        info.get("size").and_then(Value::as_u64),
        Some(u64::MAX),
        "the decoded size declarations exceed u64 by one and the summary saturates"
    );
    assert_eq!(
        info.get("compressed_size").and_then(Value::as_u64),
        Some(3),
        "the stored declarations still sum to the three actual stored bytes"
    );
    let marker = info
        .pointer("/fieldOrigins/~1size")
        .ok_or("declared size origin")?;
    assert_eq!(
        marker.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "saturation does not drop either original size declaration"
    );
    let references = marker
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("declared size uses")?;
    let inventory = info
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("inventory uses")?;
    assert!(
        inventory.iter().any(|site| {
            site.get("id").is_some_and(|id| references.contains(id))
                && site.get("scope").and_then(Value::as_str) == Some("archiveDirectoryMetadata")
                && site.get("role").and_then(Value::as_str) == Some("conversion")
                && site.pointer("/location/field").and_then(Value::as_str)
                    == Some("uncompressed_size")
                && site.pointer("/location/indices") == Some(&json!(["0", "2"]))
        }),
        "the result links to both original declarations and their conversion"
    );
    Ok(())
}

fn read_info(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "info"])
        .arg(path)
        .output()?;
    assert!(
        output.status.success(),
        "the information command can summarize the directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub(super) fn without_origin_companion(json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut value: Value = serde_json::from_str(json)?;
    let fields = value.as_object_mut().ok_or("Archive summary object")?;
    assert!(
        fields.remove("fieldOrigins").is_some(),
        "field-addressed layout provenance"
    );
    assert!(
        fields.remove("provenance").is_some(),
        "closed layout source inventory"
    );
    Ok(serde_json::to_string_pretty(&value)?)
}
