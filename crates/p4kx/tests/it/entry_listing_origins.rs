// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn entry_rows_bind_directory_facts_and_split_texture_members() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\group.dds", [1; 4]))
        .entry(TestEntry::deflated("Data\\group.dds.1", [2; 4]).encrypted())
        .write_to(&path)?;
    let rows = output(&["list"], &path, &[])?;
    let row = at(&rows, "/entries/0")?;
    assert_eq!(
        at(row, "/encrypted")?,
        false,
        "the header owns the decoder flag"
    );
    assert_eq!(at(row, "/size")?, 8, "the two decoded sizes are summed");
    assert_eq!(
        origin(row, "/metadata/raw_compression_method")?,
        false,
        "the root directory is plain"
    );
    assert_eq!(
        origin(row, "/size")?,
        false,
        "summing headers reads no payload bytes"
    );
    assert_eq!(
        sources(row, "/metadata/raw_compression_method")?.len(),
        1,
        "only the header owns its method"
    );
    assert_eq!(
        sources(row, "/size")?.len(),
        2,
        "both stored directory entries supply the sum"
    );
    assert_eq!(
        sources(row, "/parts/0")?.len(),
        1,
        "the path comes from its original part"
    );
    let physical = output(&["search"], &path, &["group.dds.1"])?;
    let encrypted = at(&physical, "/entries/0")?;
    assert_eq!(
        at(encrypted, "/encrypted")?,
        true,
        "physical decoder flag stays present"
    );
    assert_eq!(
        origin(encrypted, "/metadata/zip_header/general_purpose_flags")?,
        false,
        "encrypted payload and plain original directory are separate"
    );
    Ok(())
}

#[test]
fn entry_rows_retain_original_encrypted_container_ancestry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let room = TestContainer::new().entry(TestEntry::stored("room.xml", b"<Room/>".to_vec()));
    ArchiveBuilder::new()
        .entry(TestEntry::deflated("Data\\room.socpak", room.build()).encrypted())
        .write_to(&path)?;
    let rows = output(&["socpak", "list"], &path, &["Data\\room.socpak"])?;
    let entry = at(&rows, "/entries/0")?;
    assert_eq!(
        at(entry, "/encrypted")?,
        false,
        "the inner Entry is stored plain"
    );
    assert_eq!(
        origin(entry, "/metadata/dos_timestamp/raw_date")?,
        true,
        "the inner directory was read through its original encrypted container"
    );
    assert_eq!(
        origin(entry, "/size")?,
        true,
        "stored size is a sourced directory value"
    );
    assert_eq!(
        sources(entry, "/path")?.len(),
        2,
        "outer and inner original path uses"
    );
    Ok(())
}

fn output(command: &[&str], archive: &std::path::Path, extra: &[&str]) -> TestResult<Value> {
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "--key", &test_key().to_hex()])
        .args(command)
        .arg(archive)
        .args(extra)
        .output()?;
    assert!(output.status.success(), "CLI listing succeeds: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}").into())
}

fn carrier<'value>(row: &'value Value, field: &str) -> TestResult<&'value Value> {
    at(row, "/fieldOrigins")?
        .get(field)
        .ok_or_else(|| format!("missing origin for {field}").into())
}

fn origin(row: &Value, field: &str) -> TestResult<Value> {
    let source = carrier(row, field)?;
    assert_eq!(
        at(source, "/sourceSetComplete")?,
        true,
        "complete field provenance"
    );
    Ok(at(source, "/containsEncryptedSource")?.clone())
}

fn sources<'value>(row: &'value Value, field: &str) -> TestResult<Vec<&'value Value>> {
    let refs = at(carrier(row, field)?, "/sourceUsesRefs")?
        .as_array()
        .ok_or("expected source references")?;
    let uses = at(row, "/provenance/uses")?
        .as_array()
        .ok_or("expected provenance use inventory")?;
    refs.iter()
        .map(|reference| {
            let usage = uses
                .iter()
                .find(|usage| usage.get("id") == Some(reference))
                .ok_or("unresolved source-use reference")?;
            assert_eq!(
                at(usage, "/scope")?,
                &json!("archiveDirectoryMetadata"),
                "metadata uses the original directory domain"
            );
            Ok(usage)
        })
        .collect()
}

pub(crate) fn remove_checked_origins_for_snapshot(report: &mut Value) -> TestResult {
    let rows = report
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .ok_or("Entry rows")?;
    for row in rows {
        let fields = at(row, "/fieldOrigins")?
            .as_object()
            .ok_or("field-origin map")?;
        assert!(fields.contains_key("/path"), "Entry path origin");
        assert!(
            fields.contains_key("/metadata/dos_timestamp/raw_date"),
            "raw date origin"
        );
        for field in fields.keys() {
            if !field.is_empty() {
                at(row, field)?;
            }
            assert!(
                !sources(row, field)?.is_empty(),
                "each Entry field has an original use"
            );
        }
        let object = row.as_object_mut().ok_or("Entry row object")?;
        object.remove("fieldOrigins");
        object.remove("provenance");
    }
    Ok(())
}
