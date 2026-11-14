// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{
    player_bundle_manifest_schema, player_bundle_record_schema, write_player_source_bundle,
};
use p4kx_extraction::{Archive, CancelToken, OriginDigestWriter};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::player_source::fixture_with_entries;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MALFORMED: &str = "Data/Scripts/Loadouts/Player/DefaultLoadouts/malformed.xml";

#[test]
fn shared_player_bundle_validates_its_records_receipts_and_raw_source_identity() -> TestResult {
    let (_scratch, archive) =
        fixture_with_entries([TestEntry::stored(MALFORMED, br"<Loadout>").encrypted()])?;
    let workspace = tempfile::tempdir()?;
    let output = workspace.path().join("bundle");
    write_player_source_bundle(
        &archive,
        &[BodyVariant::Female, BodyVariant::Male],
        &output,
        &CancelToken::new(),
    )?;
    let manifest: Value = serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
    let manifest_validator = jsonschema::validator_for(&player_bundle_manifest_schema())?;
    manifest_validator
        .validate(&manifest)
        .map_err(|e| e.to_string())?;
    assert_eq!(
        manifest.get("bodies"),
        Some(&json!(["female", "male"])),
        "caller order"
    );
    let bytes = fs::read(output.join("records.jsonl"))?;
    compare_digest(manifest.get("records").ok_or("records descriptor")?, &bytes)?;
    let rows: Vec<Value> = std::str::from_utf8(&bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let validator = jsonschema::validator_for(&player_bundle_record_schema())?;
    for row in &rows {
        validator
            .validate(row)
            .map_err(|e| format!("{} {e}", kind(row)))?;
    }
    verify_rows(&rows, &manifest)?;
    verify_artifacts(&output, &archive, &rows, &manifest)?;
    Ok(())
}

fn verify_rows(rows: &[Value], manifest: &Value) -> TestResult {
    let [header, middle @ .., end] = rows else {
        return Err("header and end required".into());
    };
    assert_eq!(kind(header), "header", "first row");
    assert_eq!(kind(end), "end", "last row");
    assert_eq!(
        end.get("rows"),
        Some(&json!((rows.len() - 1).to_string())),
        "footer excludes itself"
    );
    assert_eq!(
        manifest.pointer("/records/rowCount"),
        Some(&json!(rows.len().to_string())),
        "all lines"
    );
    assert_eq!(
        end.get("rawArtifacts"),
        manifest.get("rawArtifacts"),
        "both closures measured"
    );
    let unique_row_kinds: BTreeSet<_> = middle.iter().map(kind).collect();
    for expected in [
        "body",
        "loadout",
        "motionActor",
        "motionComponents",
        "motionComponent",
        "motionOptions",
        "motionOption",
        "databaseGraph",
        "databaseBranch",
        "databaseOption",
        "databaseLink",
        "adbDocument",
        "adbFragment",
        "adbTagScope",
        "adbTagSelectors",
        "tagDefinitionRoot",
        "source",
        "sourceUse",
        "document",
        "documentRejection",
    ] {
        assert!(
            unique_row_kinds.contains(expected),
            "{expected} known row family"
        );
    }
    let ids: BTreeSet<_> = middle
        .iter()
        .filter(|row| kind(row) == "sourceUse")
        .filter_map(|row| row.pointer("/value/id").and_then(Value::as_str))
        .collect();
    for row in middle {
        verify_refs(row, &ids)?;
    }
    Ok(())
}

fn verify_refs(value: &Value, ids: &BTreeSet<&str>) -> TestResult {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if key == "sourceUsesRefs" {
                    for item in value.as_array().ok_or("source use list")? {
                        let id = item.as_str().ok_or("opaque use ID")?;
                        assert!(ids.contains(id), "every field origin resolves to one use");
                    }
                } else {
                    verify_refs(value, ids)?;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                verify_refs(item, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn verify_artifacts(
    root: &Path,
    archive: &Archive,
    rows: &[Value],
    manifest: &Value,
) -> TestResult {
    let sources: BTreeMap<_, _> = rows
        .iter()
        .filter(|row| kind(row) == "source")
        .filter_map(|row| Some((row.pointer("/value/id")?.as_str()?, row.get("value")?)))
        .collect();
    let mut originals = BTreeMap::new();
    let mut total = 0_u64;
    for row in rows
        .iter()
        .filter(|row| matches!(kind(row), "document" | "documentRejection"))
    {
        let source = row
            .get("source")
            .and_then(Value::as_str)
            .ok_or("artifact source")?;
        let physical = sources.get(source).ok_or("registered original")?;
        let relative = row
            .pointer("/artifact/path")
            .and_then(Value::as_str)
            .ok_or("relative output path")?;
        if let Some(previous) = originals.insert(source, relative) {
            assert_eq!(
                previous, relative,
                "one raw file per admitted physical source"
            );
            continue;
        }
        total += compare_original(root, archive, row, physical)?;
    }
    assert_eq!(
        manifest.pointer("/rawArtifacts/count"),
        Some(&json!(originals.len().to_string())),
        "unique bound artifacts"
    );
    assert_eq!(
        manifest.pointer("/rawArtifacts/byteLength"),
        Some(&json!(total.to_string())),
        "actual original bytes"
    );
    assert_eq!(
        fs::read_dir(root.join("originals"))?.count(),
        originals.len(),
        "no unauthenticated extras"
    );
    Ok(())
}

fn compare_original(
    root: &Path,
    archive: &Archive,
    row: &Value,
    source: &Value,
) -> TestResult<u64> {
    let name = source
        .pointer("/path/value")
        .and_then(Value::as_str)
        .ok_or("authored P4K path")?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == name)
        .ok_or("physical archive member")?;
    let mut original = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut original)?;
    let artifact = row.get("artifact").ok_or("artifact receipt")?;
    let path = artifact
        .get("path")
        .and_then(Value::as_str)
        .ok_or("relative output path")?;
    let actual = fs::read(root.join(path))?;
    assert_eq!(actual, original, "exact fully verified member bytes");
    let crc = crc32c::crc32c(&actual);
    assert_eq!(
        artifact.get("crc32c"),
        Some(&json!(format!("{crc:08x}"))),
        "measured CRC-32C"
    );
    compare_digest(artifact, &actual)?;
    Ok(actual.len() as u64)
}

fn compare_digest(json: &Value, bytes: &[u8]) -> TestResult {
    let mut measurement = OriginDigestWriter::new(io::sink());
    measurement.write_all(bytes)?;
    let digest = measurement.finish()?;
    assert_eq!(
        json.get("sha256"),
        Some(&json!(digest.sha256())),
        "persisted SHA-256"
    );
    assert_eq!(
        json.get("byteLength"),
        Some(&json!(digest.byte_length().to_string())),
        "persisted length"
    );
    Ok(())
}

fn kind(row: &Value) -> &str {
    row.get("type").and_then(Value::as_str).unwrap_or_default()
}
