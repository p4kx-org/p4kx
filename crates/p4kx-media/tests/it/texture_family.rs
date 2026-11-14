// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Exclusion, Filter, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat, test_key};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

const HEADER: &str = "Data/graph.dds";
const DDS: TestTextureFormat = TestTextureFormat::LegacyBgra8;

#[test]
fn family_graph_preserves_duplicates_missing_mips_and_declared_size_mismatches() -> TestResult {
    let root = tempfile::tempdir()?;
    let archive = fixture(root.path())?;
    let output = root.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        include: Some(Filter::parse("*graph.dds")?),
        exclude: Exclusion::parse(["*graph.dds.9"])?,
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    check_directory(&rows)?;
    check_expected(&rows)?;
    check_schema(&rows)?;
    check_origins(&output.join(&index.artifact_base), &rows)
}

fn fixture(root: &Path) -> TestResult<Archive> {
    let dds = SplitDds::new(DDS, 16, 8).split(3);
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(HEADER, dds.header_bytes()))
        .entry(TestEntry::stored(HEADER, dds.header_bytes()).encrypted())
        .entry(TestEntry::stored("Data/graph.dds.1", dds.part(1)))
        .entry(TestEntry::zstd("data/GRAPH.dds.1", dds.part(1)).encrypted())
        .entry(TestEntry::stored("Data/graph.dds.3", b"short"))
        .entry(TestEntry::stored("Data/graph.dds.9", b"outside layout"))
        .entry(TestEntry::stored("Data/graph.dds.1a", b"orphan alpha part"))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

const fn owner(row: &MediaRecord) -> Option<usize> {
    if let Identity::Entry { entry, .. } = &row.source.identity {
        Some(entry.entry_index)
    } else {
        None
    }
}

fn find<'a>(
    rows: &'a [MediaRecord],
    header: usize,
    namespace: &str,
    indices: &[u64],
) -> TestResult<&'a MediaRecord> {
    rows.iter()
        .find(|row| {
            owner(row) == Some(header)
                && row.subresource.namespace == namespace
                && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} {header} {indices:?}").into())
}

fn check_directory(rows: &[MediaRecord]) -> TestResult {
    for owner_index in 0..=1 {
        let family = find(rows, owner_index, "texture.dds-family", &[])?;
        assert_eq!(
            family.data.pointer("/payload/observed_members"),
            Some(&json!("7")),
            "every physical occurrence"
        );
        assert_eq!(
            family.data.pointer("/payload/directory/status"),
            Some(&json!("complete")),
            "full original directory"
        );
        assert_eq!(
            family.data.pointer("/payload/member_paths"),
            Some(&json!("lossless")),
            "lossless spellings"
        );
        for candidate in 0..7 {
            let row = find(rows, owner_index, "texture.dds-member", &[candidate])?;
            assert_eq!(
                row.data.pointer("/payload/entry/entry_index"),
                Some(&json!(candidate)),
                "exact member index"
            );
        }
    }
    check_filters(rows)
}

fn check_filters(rows: &[MediaRecord]) -> TestResult {
    let filtered = find(rows, 0, "texture.dds-member", &[5])?;
    assert_eq!(
        filtered.data.pointer("/payload/selection/status"),
        Some(&json!("not-selected")),
        "exclusion stays explicit"
    );
    assert_eq!(
        filtered.data.pointer("/payload/declared_bytes"),
        Some(&json!("14")),
        "size remains directory metadata"
    );
    assert_eq!(
        filtered.data.pointer("/payload/payload_access"),
        Some(&json!("not-inspected")),
        "no read claim"
    );
    assert_eq!(
        filtered.data.pointer("/payload/filter"),
        Some(&json!("excluded")),
        "explicit exclusion"
    );
    let unmatched = find(rows, 0, "texture.dds-member", &[2])?;
    assert_eq!(
        unmatched.data.pointer("/payload/filter"),
        Some(&json!("unmatched")),
        "include mismatch is distinct"
    );
    let own = find(rows, 0, "texture.dds-member", &[0])?;
    assert_eq!(
        own.data.pointer("/payload/filter"),
        Some(&json!("included")),
        "selected header belongs to graph"
    );
    let orphan = find(rows, 0, "texture.dds-member", &[6])?;
    assert_eq!(
        orphan.data.pointer("/payload/relation/role"),
        Some(&json!("alpha-part")),
        "alpha orphan is retained"
    );
    Ok(())
}

fn check_expected(rows: &[MediaRecord]) -> TestResult {
    for header in 0..=1 {
        for (part, level, count, disposition) in [
            (1, 2, "2", "ambiguous"),
            (2, 1, "0", "missing"),
            (3, 0, "1", "declared-size-mismatch"),
        ] {
            let row = find(rows, header, "texture.dds-expected-part", &[part])?;
            assert_eq!(
                row.status,
                Status::Complete,
                "classification is observed even if source layout is incomplete"
            );
            assert_eq!(
                row.data.pointer("/payload/level"),
                Some(&json!(level)),
                "DDS part/mip direction"
            );
            assert_eq!(
                row.data.pointer("/payload/matching_members"),
                Some(&json!(count)),
                "no candidate collapse"
            );
            assert_eq!(
                row.data.pointer("/payload/disposition"),
                Some(&json!(disposition)),
                "exact physical state"
            );
        }
    }
    Ok(())
}

fn check_schema(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        let wire = serde_json::to_value(row)?;
        validator
            .validate(&wire)
            .map_err(|error| error.to_string())?;
    }
    let member = find(rows, 0, "texture.dds-member", &[2])?;
    let mut invalid = serde_json::to_value(member)?;
    *invalid
        .pointer_mut("/subresource/indices")
        .ok_or("member coordinate")? = json!([]);
    assert!(
        !validator.is_valid(&invalid),
        "physical member needs one index"
    );
    Ok(())
}

fn check_origins(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let fields: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    for (index, encrypted) in [(2, false), (3, false)] {
        let row = find(rows, 0, "texture.dds-member", &[index])?;
        let ordinal = rows
            .iter()
            .position(|item| std::ptr::eq(item, row))
            .ok_or("row position")?;
        let field = field(&fields, ordinal, "/data/payload/declared_bytes")?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "plaintext directory metadata is separate from payload encryption"
        );
    }
    let second = find(rows, 1, "texture.dds-expected-part", &[2])?;
    let ordinal = rows
        .iter()
        .position(|item| std::ptr::eq(item, second))
        .ok_or("row position")?;
    let proof = field(&fields, ordinal, "/data/payload/expected_bytes")?;
    assert_eq!(
        proof.get("containsEncryptedSource"),
        Some(&json!(true)),
        "expected length consumes the exact encrypted owner header"
    );
    assert_eq!(
        proof.get("sourceSetComplete"),
        Some(&json!(true)),
        "exact original layout evidence"
    );
    Ok(())
}

fn field<'a>(fields: &'a [Value], ordinal: usize, path: &str) -> TestResult<&'a Value> {
    let ordinal = ordinal.to_string();
    fields
        .iter()
        .find(|row| {
            row.get("field").and_then(Value::as_str) == Some(path)
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("texture")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
        })
        .ok_or_else(|| format!("missing field receipt for {ordinal} {path}").into())
}

#[test]
fn family_graph_does_not_adopt_lossy_names_or_open_filtered_encrypted_siblings() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    let dds = SplitDds::new(DDS, 4, 4).split(1);
    let header = "Data/bad\u{fffd}.dds";
    ArchiveBuilder::new()
        .entry(TestEntry::stored(header, dds.header_bytes()))
        .entry(TestEntry::stored("unused", b"lossy").with_path_bytes(b"Data/bad\xff.dds.1"))
        .entry(TestEntry::stored("Data/bad\u{fffd}.dds.2", b"opaque").encrypted())
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = root.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        include: Some(Filter::parse(&format!("*{header}"))?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    let lossy = find(&rows, 0, "texture.dds-member", &[1])?;
    assert_eq!(
        lossy.data.pointer("/payload/relation/role"),
        Some(&json!("unproved-path")),
        "display name does not prove physical membership"
    );
    let key_required = find(&rows, 0, "texture.dds-member", &[2])?;
    assert_eq!(
        key_required.data.pointer("/payload/payload_access"),
        Some(&json!("key-required")),
        "explicit missing key without read"
    );
    let unresolved = find(&rows, 0, "texture.dds-expected-part", &[1])?;
    assert_eq!(
        unresolved.data.pointer("/payload/disposition"),
        Some(&json!("unproved")),
        "lossy family cannot prove missing"
    );
    Ok(())
}
