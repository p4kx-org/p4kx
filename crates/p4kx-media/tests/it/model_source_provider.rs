// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::support::{self, TestResult};

const STREAM: &str = "model.jsonl";
const UNKNOWN: u32 = 0x0BAD_BEEF;

#[test]
fn model_provider_retains_every_physical_chunk_and_its_immediate_origin() -> TestResult {
    let temp = tempfile::tempdir()?;
    let file = TestChunkFile::ivo()
        .chunk(
            ivo::PHYSICAL_HIERARCHY.id(),
            0x900,
            b"synthetic opaque physics",
        )
        .chunk(UNKNOWN, 0x901, b"unknown bytes")
        .bytes();
    let archive = duplicate_model_archive(temp.path(), &file)?;
    let output = temp.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let base = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, STREAM)?;
    assert_eq!(
        rows.len(),
        6,
        "two physical model entries, no animation rows"
    );
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "model row satisfies the closed schema"
        );
    }
    let original = ChunkFile::parse(file.as_slice())?;
    for owner in [0, 1] {
        let owned = rows.iter().enumerate().filter(|(_, row)| {
            matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        }).collect::<Vec<_>>();
        let [(_, entry), (known_ordinal, known), (_, unknown)] = owned.as_slice() else {
            return Err("expected one model entry and two source chunks".into());
        };
        assert_eq!(entry.subresource.namespace, "model-entry");
        assert_eq!(entry.data.pointer("/payload/family"), Some(&json!("cgf")));
        assert_eq!(entry.data.pointer("/payload/chunk_count"), Some(&json!(2)));
        let raw = entry
            .source
            .raw
            .as_ref()
            .ok_or("original retained payload")?;
        assert_eq!(fs::read(base.join(&raw.path))?, file);
        assert_chunk(known, &original, 0, "known-opaque")?;
        assert_chunk(unknown, &original, 1, "unknown-type")?;
        assert_eq!(known.source.raw.as_ref(), Some(raw));
        assert_eq!(unknown.source.raw.as_ref(), Some(raw));
        assert_chunk_origins(&base, *known_ordinal, &original, owner)?;
    }
    Ok(())
}

fn duplicate_model_archive(folder: &Path, file: &[u8]) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Synthetic/source.CGF", file))
        .entry(TestEntry::zstd("Synthetic/source.CGF", file).encrypted())
        .entry(TestEntry::stored("Synthetic/source.caf", file))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn assert_chunk(
    row: &MediaRecord,
    original: &ChunkFile<&[u8]>,
    index: usize,
    projection: &str,
) -> TestResult {
    assert_eq!(
        row.subresource.namespace, "model-chunk",
        "chunk occurrence namespace"
    );
    assert_eq!(
        row.subresource.indices,
        vec![index as u64],
        "physical table ordinal"
    );
    assert_eq!(
        row.status,
        Status::Complete,
        "opaque byte range is complete"
    );
    assert_eq!(
        row.data.pointer("/payload/projection"),
        Some(&json!(projection)),
        "known and unknown type dispositions"
    );
    let original = original
        .chunks()
        .get(index)
        .ok_or("original chunk table row")?;
    assert_eq!(
        row.data.pointer("/payload/type_id"),
        Some(&json!(original.chunk_type().id())),
        "physical type word"
    );
    assert_eq!(
        row.data.pointer("/payload/version"),
        Some(&json!(original.version())),
        "physical version word"
    );
    assert_eq!(
        row.data.pointer("/payload/range/start"),
        Some(&json!(original.offset())),
        "original inclusive byte start"
    );
    assert_eq!(
        row.data.pointer("/payload/range/end_exclusive"),
        Some(&json!(original.offset() + original.size())),
        "original exclusive byte end",
    );
    Ok(())
}

fn assert_chunk_origins(
    base: &Path,
    ordinal: usize,
    original: &ChunkFile<&[u8]>,
    owner: usize,
) -> TestResult {
    let row = original.table_row(0).ok_or("original first row")?;
    for (field, name, range) in [
        ("type_id", "modelChunkType", row.kind),
        ("range/start", "modelChunkOffset", row.offset.clone()),
        (
            "range/end_exclusive",
            "modelChunkExtentTable",
            original.table_range(),
        ),
        ("range", "modelChunkOffset", row.offset),
    ] {
        let uses = field_uses(
            base,
            ordinal,
            &format!("/data/payload/{field}"),
            owner,
            owner == 1,
        )?;
        assert_exact_use(&uses, name, range);
    }
    Ok(())
}

fn assert_exact_use(uses: &[Value], field: &str, range: Range<u64>) {
    let location = json!({
        "type": "subresource",
        "namespace": "model-byte-range",
        "indices": [range.start.to_string(), range.end.to_string()],
        "field": field
    });
    assert!(
        uses.iter()
            .any(|usage| usage.get("location") == Some(&location)),
        "actual source use includes exactly the original bytes of {field}"
    );
}

pub(super) fn field_uses(
    base: &Path,
    ordinal: usize,
    field: &str,
    owner: usize,
    encrypted: bool,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let ordinal = ordinal.to_string();
    let data = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let row = data
        .lines()
        .map(serde_json::from_str::<Value>)
        .find(|row| {
            row.as_ref().is_ok_and(|row| {
                row.get("field").and_then(Value::as_str) == Some(field)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("model")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
            })
        })
        .ok_or_else(|| format!("actual model field origin for {field}"))??;
    assert_eq!(
        row.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "encryption is specific to the original Entry"
    );
    assert_eq!(
        row.get("sourceSetComplete"),
        Some(&json!(true)),
        "the byte range source is completely known"
    );
    let references = row
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("actual uses")?;
    let uses = json_rows(base, "origin-uses.jsonl")?;
    let sources = json_rows(base, "origin-sources.jsonl")?;
    references
        .iter()
        .map(|reference| {
            let usage = uses
                .iter()
                .find(|usage| usage.get("id") == Some(reference))
                .ok_or("closed original source use")?;
            let source = sources
                .iter()
                .find(|source| source.get("id") == usage.get("source"))
                .ok_or("closed physical model source")?;
            assert_eq!(
                source.get("entryIndex"),
                Some(&json!(owner)),
                "an identical source path cannot replace the original Entry"
            );
            Ok(usage.clone())
        })
        .collect()
}

fn json_rows(base: &Path, file: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(base.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

#[test]
fn malformed_model_table_retains_raw_only_in_a_diagnostic_generation() -> TestResult {
    let temp = tempfile::tempdir()?;
    let original = b"#ivo damaged";
    let archive = support::archive_with(temp.path(), [TestEntry::stored("bad.cga", original)])?;
    let output = temp.path().join("out");
    let outcome = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            diagnostic: true,
            ..options()
        },
    )?;
    let ExportOutcome::Incomplete {
        index, diagnostic, ..
    } = outcome
    else {
        return Err("damaged model table should be explicit partial output".into());
    };
    let root = diagnostic.parent().ok_or("diagnostic generation base")?;
    let lines = fs::read_to_string(root.join(STREAM))?;
    let row: MediaRecord = serde_json::from_str(lines.lines().next().ok_or("model entry row")?)?;
    assert_eq!(row.status, Status::Partial);
    assert_eq!(row.data.pointer("/payload/chunk_count"), Some(&Value::Null));
    let raw = row
        .source
        .raw
        .as_ref()
        .ok_or("retained malformed payload")?;
    assert_eq!(fs::read(root.join(&raw.path))?, original);
    assert!(
        index.field_origins.is_some(),
        "diagnostic retains scoped field origins"
    );
    assert!(!output.join("media-index.json").exists());
    Ok(())
}

#[test]
fn a_physical_model_row_failure_links_the_authored_fault_instead_of_only_its_header() -> TestResult
{
    let temp = tempfile::tempdir()?;
    let mut original = TestChunkFile::crch()
        .chunk(1, 1, b"valid data")
        .big_endian_chunk(2, 2, b"invalid extent")
        .bytes();
    let valid = ChunkFile::parse(original.as_slice())?;
    let row = valid.table_row(1).ok_or("original second row")?;
    let original_row = row.kind.start..row.offset.end;
    let offset = usize::try_from(row.offset.start)?..usize::try_from(row.offset.end)?;
    original
        .get_mut(offset)
        .ok_or("original offset word")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let archive = support::archive_with(temp.path(), [TestEntry::stored("bad.chrm", original)])?;
    let output = temp.path().join("out");
    let outcome = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            diagnostic: true,
            ..options()
        },
    )?;
    let ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("invalid physical row requires diagnostic output".into());
    };
    let root = diagnostic.parent().ok_or("diagnostic generation base")?;
    for path in ["/status", "/issues/0/detail"] {
        assert_exact_use(
            &field_uses(root, 0, path, 0, false)?,
            "modelChunkRowFailure",
            original_row.clone(),
        );
    }
    Ok(())
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    }
}
