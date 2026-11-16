// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::{Receipt, TestResult, artifact, index, verify};

const FRAME: &str = "p4kx-catalog.json.zst";

#[test]
fn raw_and_zstd_replay_match_without_a_raw_artifact() -> TestResult {
    let raw = compact_fixture(&catalog())?;
    let directory = tempfile::tempdir()?;
    let compressed = frame(&raw)?;
    fs::write(directory.path().join(FRAME), &compressed)?;
    write_receipt(directory.path(), &raw, &compressed)?;
    let binding = Receipt::read(directory.path())?.binding()?;

    let raw_index = index(raw.as_slice())?;
    let compressed_index = artifact::read_completed(directory.path(), |input| index(input))?;
    assert_eq!(
        raw_index.digest, compressed_index.digest,
        "the two inputs represent the same exact snapshot"
    );
    assert_eq!(
        raw_index.sections.decoded_bytes,
        raw.len() as u64,
        "section counts use the original decoded bytes"
    );
    assert_eq!(
        (
            compressed_index.sections.physical_sources,
            compressed_index.sections.source_uses
        ),
        (3, 4),
        "the replay counts the whole inventory, including an unreferenced attempt"
    );

    let plain = verify(raw.as_slice(), &raw_index, &binding)?;
    let replay = artifact::read_completed(directory.path(), |input| {
        verify(input, &compressed_index, &binding)
    })?;
    assert_eq!(
        plain, replay,
        "all row and sourcing counts agree with the raw audit"
    );
    assert_eq!(
        (
            replay.items,
            replay.item_definitions,
            replay.blueprint_definitions,
            replay.container_instances
        ),
        (Some(1), Some(2), Some(1), Some(2)),
        "both physical occurrences of one nominal GUID and all other selected rows survive"
    );
    assert_eq!(
        replay.sourced_cells, 6,
        "the encrypted, clear and unknown nested values were checked"
    );
    assert!(
        !directory.path().join("p4kx-catalog.json").exists(),
        "offline replay does not create a raw file"
    );
    Ok(())
}

#[test]
fn recorded_identity_rejects_wrong_build_and_mixed_replay_bytes() -> TestResult {
    let original = catalog();
    let raw = compact_fixture(&original)?;
    let receipt: Receipt = serde_json::from_value(receipt(&raw, &raw))?;
    let binding = receipt.binding()?;

    let mut wrong_build = original.clone();
    *path(&mut wrong_build, "/source/build/changeNumber")? = json!("2");
    let body = compact_fixture(&wrong_build)?;
    let wrong_index = index(body.as_slice())?;
    let error =
        verify(body.as_slice(), &wrong_index, &binding).expect_err("different build identity");
    assert!(
        error.to_string().contains("source identity"),
        "a valid snapshot with a different Build is refused"
    );

    let first = index(raw.as_slice())?;
    let mut other = original;
    *path(&mut other, "/definitions/items/rows/0/name/value")? = json!("different synthetic data");
    let other = compact_fixture(&other)?;
    let error = verify(other.as_slice(), &first, &binding).expect_err("changed second pass");
    assert!(
        error.to_string().contains("between validation passes"),
        "valid replay rows cannot switch generations mid-audit"
    );
    Ok(())
}

#[test]
fn corrupt_physical_and_selector_links_fail_before_row_replay() -> TestResult {
    for (pointer, changed, expected) in [
        (
            "/provenance/uses/0/source",
            json!("missing"),
            "missing physical source",
        ),
        (
            "/provenance/sources/1/ancestors/0",
            json!("missing"),
            "missing physical source ancestor",
        ),
        (
            "/provenance/uses/1/location/selector",
            json!("missing"),
            "missing localization selector",
        ),
        (
            "/provenance/uses/1/location/selector",
            json!("u-attempt"),
            "selector role",
        ),
    ] {
        let mut document = catalog();
        *path(&mut document, pointer)? = changed;
        let body = compact_fixture(&document)?;
        let Err(error) = index(body.as_slice()) else {
            return Err(format!("expected a corrupt provenance failure at {pointer}").into());
        };
        assert!(
            error.to_string().contains(expected),
            "wrong failure at {pointer}: {error}"
        );
    }
    Ok(())
}

#[test]
fn bad_markers_dangling_fields_and_duplicate_use_refs_fail_in_stream() -> TestResult {
    let parent = "/definitions/items/rows/0/name";
    for (property, changed, expected) in [
        ("containsEncryptedSource", json!(false), "marker disagrees"),
        (
            "sourceUsesRefs",
            json!(["u-missing"]),
            "missing contributing source use",
        ),
        (
            "sourceUsesRefs",
            json!(["u-key", "u-key"]),
            "sorted and unique",
        ),
        ("sourceUsesRefs", json!(["u-attempt"]), "noncontributing"),
    ] {
        let mut document = catalog();
        *path(&mut document, &format!("{parent}/{property}"))? = changed;
        let raw = compact_fixture(&document)?;
        let typed: Receipt = serde_json::from_value(receipt(&raw, &raw))?;
        let binding = typed.binding()?;
        let indexed = index(raw.as_slice())?;
        let error = verify(raw.as_slice(), &indexed, &binding).expect_err("invalid cell evidence");
        assert!(
            error.to_string().contains(expected),
            "wrong failure for {property}: {error}"
        );
    }
    Ok(())
}

#[test]
fn truncated_checksum_and_json_cannot_pass_replay() -> TestResult {
    let mut raw = compact_fixture(&catalog())?;
    let directory = tempfile::tempdir()?;
    let mut compressed = frame(&raw)?;
    compressed.pop().ok_or("fixture frame has a checksum")?;
    fs::write(directory.path().join(FRAME), &compressed)?;
    write_receipt(directory.path(), &raw, &compressed)?;
    assert!(
        artifact::read_completed(directory.path(), |input| index(input)).is_err(),
        "a truncated frame fails even when the encoded-length receipt agrees"
    );
    raw.truncate(raw.len() - 4);
    assert!(
        index(raw.as_slice()).is_err(),
        "truncated original JSON is refused"
    );
    Ok(())
}

fn catalog() -> Value {
    let complete: serde_json::Map<String, Value> = Kind::ALL
        .into_iter()
        .map(|kind| (kind.name().to_owned(), json!({"status": "complete"})))
        .collect();
    json!({
        "schemaVersion": 3,
        "source": {
            "channel": "LIVE", "build": build(),
            "archiveSize": 47, "archiveModified": "2026-11-12T07:12:02Z"
        },
        "exportHealth": {
            "status": "complete", "read": "complete", "projection": "complete", "publicationMode": "normal"
        },
        "completeness": complete,
        "items": [{"guid": "equipment"}],
        "definitions": {
            "items": table(&json!([
                {"recordIndex": "4", "guid": "equipment", "name": cell(&json!("part"), Some(true), true, &["u-key", "u-lookup"])},
                {"recordIndex": "5", "guid": "equipment", "name": cell(&json!("part alias"), Some(true), true, &["u-key"])}
            ])),
            "blueprints": table(&json!([
                {"itemGuid": cell(&json!("equipment"), Some(true), true, &["u-key"])}
            ]))
        },
        "placements": {
            "containerInstances": table(&json!([
                {"instanceId": cell(&json!(["root"]), Some(false), true, &["u-lookup"])},
                {"parentInstanceId": cell(&json!(["root"]), None, false, &["u-opaque"])}
            ]))
        },
        "texts": {"gameTexts": [{"translations": cell(&json!({"en": "fixture"}), Some(true), true, &["u-key", "u-lookup"])}]},
        "provenance": {
            "sources": [{"id": "outer", "ancestors": []}, {"id": "dcb", "ancestors": ["outer"]}, {"id": "ini", "ancestors": []}],
            "uses": [
                {"id": "u-key", "source": "dcb", "role": "selector", "effectiveEncryption": "encrypted", "location": {"type": "dataForge"}},
                {"id": "u-lookup", "source": "ini", "role": "payload", "effectiveEncryption": "unencrypted", "location": {"type": "localizationLookup", "selector": "u-key"}},
                {"id": "u-opaque", "source": "dcb", "role": "conversion", "effectiveEncryption": "unknown", "location": {"type": "opaque"}},
                {"id": "u-attempt", "source": "dcb", "role": "attempted", "effectiveEncryption": "encrypted", "location": {"type": "dataForge"}}
            ]
        }
    })
}

fn table(rows: &Value) -> Value {
    json!({"availability": "available", "rows": rows})
}

fn cell(value: &Value, encrypted: Option<bool>, complete: bool, references: &[&str]) -> Value {
    json!({
        "value": value, "state": "present", "containsEncryptedSource": encrypted,
        "sourceSetComplete": complete, "sourceUsesRefs": references
    })
}

fn build() -> Value {
    json!({"branch": "fixture", "version": "5.0.0", "changeNumber": "1", "buildId": "fixture-1"})
}

fn receipt(raw: &[u8], compressed: &[u8]) -> Value {
    let mut build = build();
    build["channel"] = json!("LIVE");
    let stamp = json!({
        "manifestSha256": "0".repeat(64), "build": build, "archiveBytes": 47,
        "archiveCreatedUtc": "2026-11-12T07:00:00.0000000Z",
        "archiveModifiedUtc": "2026-11-12T07:12:02.1234567Z"
    });
    json!({
        "status": "complete", "mode": "normal", "channel": "LIVE",
        "archiveBefore": &stamp, "archiveAfter": &stamp,
        "sizes": {"encodedBytes": compressed.len(), "decodedBytes": raw.len(), "fileBytes": compressed.len()}
    })
}

fn compact_fixture(value: &Value) -> TestResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn frame(raw: &[u8]) -> TestResult<Vec<u8>> {
    let mut encoder = zstd::Encoder::new(Vec::new(), 1)?;
    encoder.include_checksum(true)?;
    encoder.write_all(raw)?;
    Ok(encoder.finish()?)
}

fn write_receipt(directory: &std::path::Path, raw: &[u8], compressed: &[u8]) -> TestResult {
    fs::write(
        directory.join("receipt.private.json"),
        serde_json::to_vec(&receipt(raw, compressed))?,
    )?;
    Ok(())
}

fn path<'fixture>(value: &'fixture mut Value, pointer: &str) -> TestResult<&'fixture mut Value> {
    value
        .pointer_mut(pointer)
        .ok_or_else(|| format!("missing fixture path {pointer}").into())
}
