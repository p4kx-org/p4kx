// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};
use serde_json::{Value, json};

use super::{DATA, json_of, p4kx};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn verify_json_binds_duplicate_paths_to_each_physical_checksum() -> TestResult {
    const PATH: &str = "Data\\duplicate.bin";
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, b""))
        .entry(TestEntry::deflated(PATH, DATA).with_crc32c(1))
        .entry(TestEntry::zstd(PATH, DATA).with_crc32c(2))
        .write_to(&archive)?;

    let report = output(&["verify"], &archive, &[])?;
    assert_eq!(report["verified"], 1, "one physical entry passed");
    assert_eq!(
        report
            .get("mismatched")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "two physical entries failed with a checksum mismatch"
    );
    for (row, expected) in ["00000001", "00000002"].into_iter().enumerate() {
        let pointer = format!("/mismatched/{row}");
        assert_eq!(
            at(&report, &format!("{pointer}/path"))?,
            PATH,
            "same literal path"
        );
        assert_eq!(
            at(&report, &format!("{pointer}/kind"))?,
            "crc32c",
            "decoded CRC kind"
        );
        assert_eq!(
            at(&report, &format!("{pointer}/expected"))?,
            expected,
            "original CRC"
        );
        assert_crc_origins(&report, &pointer, row + 1, DATA.len())?;
    }
    assert_marker(&report, "/verified", false, true)?;
    let success = hash_use(&report, "/verified", "entryDecodedBytes", 0)?;
    assert_entry(&report, success, 0)
}

#[test]
fn verify_json_sha256_uses_the_exact_stored_extent() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let entry = TestEntry::zstd("Data\\stored.bin", DATA).with_sha256_value([0xAB; 32]);
    let stored_len = entry.payload().len();
    assert_ne!(
        stored_len,
        DATA.len(),
        "fixture distinguishes the byte domains"
    );
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\checked.bin", DATA).with_sha256())
        .entry(entry)
        .write_to(&archive)?;

    let report = output(&["verify"], &archive, &[])?;
    assert_eq!(report["verified"], 1, "one stored checksum passes");
    let verified = hash_use(&report, "/verified", "entryStoredBytes", stored_len)?;
    assert_entry(&report, verified, 0)?;
    assert_eq!(
        at(&report, "/mismatched/0/kind")?,
        "sha256",
        "stored SHA kind"
    );
    assert_eq!(
        at(&report, "/mismatched/0/expected")?,
        &json!("ab".repeat(32)),
        "the wrong declared digest stays visible"
    );
    assert_entry(
        &report,
        directory_use(&report, "/mismatched/0/expected", "sha256", "conversion")?,
        1,
    )?;
    assert_entry(
        &report,
        directory_use(&report, "/mismatched/0/kind", "sha256", "selector")?,
        1,
    )?;
    assert_marker(&report, "/mismatched/0/actual", false, true)?;
    assert_entry(
        &report,
        hash_use(
            &report,
            "/mismatched/0/actual",
            "entryStoredBytes",
            stored_len,
        )?,
        1,
    )
}

#[test]
fn verify_json_unreadable_entry_has_no_completed_checksum_evidence() -> TestResult {
    const PATH: &str = "Data\\truncated.bin";
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let entry = TestEntry::deflated(PATH, DATA);
    let prefix = entry.payload().get(..4).ok_or("deflate prefix")?.to_vec();
    ArchiveBuilder::new()
        .entry(entry.with_payload(prefix))
        .write_to(&archive)?;

    let report = output(&["verify"], &archive, &[])?;
    assert_eq!(
        report["verified"], 0,
        "the decoder fails before verification completes"
    );
    assert_eq!(
        report["mismatched"],
        json!([]),
        "there is no full checksum mismatch"
    );
    assert_eq!(
        at(&report, "/failed/0/path")?,
        PATH,
        "the failed path remains"
    );
    assert_entry(
        &report,
        directory_use(&report, "/failed/0/path", "path", "payload")?,
        0,
    )?;
    assert_eq!(
        origin(&report, "/failed/0/error")?.get("sourceSetComplete"),
        Some(&json!(false)),
        "the source set describes a failed read"
    );
    for usage in uses(&report, "/failed/0/error")? {
        assert!(
            !(usage["scope"] == "payload" && usage["role"] == "conversion"),
            "a failed read cannot supply a completed checksum"
        );
    }
    Ok(())
}

#[test]
fn verify_json_inherits_encryption_from_the_original_container() -> TestResult {
    const CONTAINER: &str = "Data\\room.socpak";
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let room =
        TestContainer::new().entry(TestEntry::deflated("room\\leaf.bin", DATA).with_crc32(1));
    ArchiveBuilder::new()
        .entry(TestEntry::deflated(CONTAINER, room.build()).encrypted())
        .write_to(&archive)?;

    let report = output(&["socpak", "verify"], &archive, &[CONTAINER])?;
    assert_eq!(
        at(&report, "/mismatched/0/path")?,
        "room\\leaf.bin",
        "stored inner path"
    );
    assert_eq!(at(&report, "/mismatched/0/kind")?, "crc32", "ZIP CRC kind");
    let outer_use = directory_use(&report, "/archive", "path", "conversion")?;
    let outer = source(&report, outer_use)?;
    assert_eq!(
        at(outer, "/path/value")?,
        CONTAINER,
        "the selected archive path"
    );
    assert_marker(&report, "/archive", false, true)?;
    for field in ["path", "expected", "actual"] {
        assert_marker(&report, &format!("/mismatched/0/{field}"), true, true)?;
    }
    let hash = hash_use(
        &report,
        "/mismatched/0/actual",
        "entryDecodedBytes",
        DATA.len(),
    )?;
    let leaf = source(&report, hash)?;
    assert_eq!(
        at(leaf, "/path/value")?,
        "room\\leaf.bin",
        "the actual inner source"
    );
    assert_eq!(
        leaf["ancestors"],
        json!([outer["id"]]),
        "exact original parent identity"
    );
    Ok(())
}

fn assert_crc_origins(report: &Value, pointer: &str, entry: usize, count: usize) -> TestResult {
    let expected = format!("{pointer}/expected");
    let actual = format!("{pointer}/actual");
    let kind = format!("{pointer}/kind");
    assert_marker(report, &expected, false, true)?;
    assert_marker(report, &actual, false, true)?;
    assert_entry(
        report,
        directory_use(report, &expected, "checksum", "conversion")?,
        entry,
    )?;
    assert_entry(
        report,
        directory_use(report, &kind, "checksum", "selector")?,
        entry,
    )?;
    assert_entry(
        report,
        hash_use(report, &actual, "entryDecodedBytes", count)?,
        entry,
    )
}

fn output(command: &[&str], archive: &Path, extra: &[&str]) -> TestResult<Value> {
    let output = p4kx()?
        .args(["--json", "--key", &test_key().to_hex()])
        .args(command)
        .arg(archive)
        .args(extra)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(1),
        "reports at least one failure: {output:?}"
    );
    json_of(&output.stdout)
}

fn at<'value>(value: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing {pointer}").into())
}

fn origin<'value>(report: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    at(report, "/fieldOrigins")?
        .get(pointer)
        .ok_or_else(|| format!("missing origin for {pointer}").into())
}

fn assert_marker(report: &Value, pointer: &str, encrypted: bool, complete: bool) -> TestResult {
    let origin = origin(report, pointer)?;
    assert_eq!(
        origin["sourceSetComplete"], complete,
        "source set of {pointer}"
    );
    assert_eq!(
        origin["containsEncryptedSource"], encrypted,
        "encryption of {pointer}"
    );
    Ok(())
}

fn uses<'value>(report: &'value Value, pointer: &str) -> TestResult<Vec<&'value Value>> {
    let ids = origin(report, pointer)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field has no source use list")?;
    let all = at(report, "/provenance/uses")?
        .as_array()
        .ok_or("source use inventory")?;
    ids.iter()
        .map(|id| {
            all.iter()
                .find(|usage| usage.get("id") == Some(id))
                .ok_or_else(|| format!("unresolved source use {id} for {pointer}").into())
        })
        .collect()
}

fn directory_use<'value>(
    report: &'value Value,
    pointer: &str,
    field: &str,
    role: &str,
) -> TestResult<&'value Value> {
    let location = json!({"type": "metadata", "field": field});
    uses(report, pointer)?
        .into_iter()
        .find(|usage| {
            usage.get("location") == Some(&location)
                && usage.get("role").and_then(Value::as_str) == Some(role)
                && usage.get("scope").and_then(Value::as_str) == Some("archiveDirectoryMetadata")
        })
        .ok_or_else(|| format!("missing original directory use {field} for {pointer}").into())
}

fn hash_use<'value>(
    report: &'value Value,
    pointer: &str,
    namespace: &str,
    end: usize,
) -> TestResult<&'value Value> {
    let location = json!({
        "type": "subresource",
        "namespace": namespace,
        "indices": ["0", &end.to_string()],
        "field": "checksum"
    });
    uses(report, pointer)?
        .into_iter()
        .find(|usage| {
            usage.get("location") == Some(&location)
                && usage.get("scope").and_then(Value::as_str) == Some("payload")
                && usage.get("role").and_then(Value::as_str) == Some("conversion")
        })
        .ok_or_else(|| format!("missing original extent {namespace} 0..{end} for {pointer}").into())
}

fn source<'value>(report: &'value Value, usage: &Value) -> TestResult<&'value Value> {
    let id = at(usage, "/source")?;
    at(report, "/provenance/sources")?
        .as_array()
        .ok_or("physical source inventory")?
        .iter()
        .find(|source| source.get("id") == Some(id))
        .ok_or_else(|| format!("unresolved physical source {id}").into())
}

fn assert_entry(report: &Value, usage: &Value, index: usize) -> TestResult {
    let source = source(report, usage)?;
    let position = at(source, "/entryIndex")?;
    let numeric = position
        .as_u64()
        .or_else(|| position.as_str()?.parse().ok());
    assert_eq!(
        numeric,
        Some(u64::try_from(index)?),
        "the observed Entry has physical index {index}"
    );
    Ok(())
}
