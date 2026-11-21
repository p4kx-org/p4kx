// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, test_key};
use serde_json::Value;

use super::support::TestResult;

#[test]
fn malformed_dba_block_keeps_only_the_uncovered_physical_metadata_rows() -> TestResult {
    let files = tempfile::tempdir()?;
    let dba = malformed_database()?;
    let archive_path = files.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("source.dba", dba.as_slice()).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = files.path().join("output");
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        diagnostic: true,
        ..ExportOptions::default()
    };
    let outcome = p4kx_media::export(&archive, &output, &options)?;
    let ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("malformed block must keep the generation incomplete".into());
    };
    assert!(
        !output.join("media-index.json").exists(),
        "diagnostics never promote an incomplete index"
    );
    let base = diagnostic
        .parent()
        .ok_or("diagnostic generation location")?;
    let rows: Vec<MediaRecord> = fs::read_to_string(base.join("animation.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    verify_rows(&rows, &dba)?;
    let index = rows
        .iter()
        .position(|row| row.subresource.namespace == "animation-dba-metadata")
        .ok_or("metadata row")?;
    verify_origins(base, index)
}

fn malformed_database() -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = TestDba::new()
        .animation(TestAnimation::new("first.caf", 30))
        .animation(TestAnimation::new("second.caf", 60))
        .animation(TestAnimation::new("second.caf", 0))
        .bytes();
    let file = ChunkFile::parse(bytes.as_slice())?;
    let (index, chunk) = file
        .chunks_of(ivo::DBA_DATA)
        .next()
        .ok_or("DbaData chunk")?;
    let mut data = file.chunk_data(index).ok_or("DbaData bytes")?.to_vec();
    *data.get_mut(16).ok_or("second block signature")? = 0;
    let (index, metadata) = file
        .chunks_of(ivo::DBA_METADATA)
        .next()
        .ok_or("DbaMetadata chunk")?;
    Ok(TestChunkFile::ivo()
        .chunk(ivo::DBA_DATA.id(), chunk.version(), data)
        .chunk(
            ivo::DBA_METADATA.id(),
            metadata.version(),
            file.chunk_data(index).ok_or("metadata bytes")?.to_vec(),
        )
        .bytes())
}

fn verify_rows(rows: &[MediaRecord], input: &[u8]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut physical = Vec::new();
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        let [ordinal] = row.subresource.indices.as_slice() else {
            continue;
        };
        let projection = row.subresource.namespace.as_str();
        if matches!(projection, "animation-clip" | "animation-dba-metadata") {
            physical.push((projection, *ordinal));
        }
        if projection == "animation-dba-metadata" {
            verify_metadata(row, *ordinal, input)?;
        }
    }
    assert_eq!(
        physical,
        [
            ("animation-clip", 0),
            ("animation-dba-metadata", 1),
            ("animation-dba-metadata", 2),
        ],
        "decoded prefix and uncovered metadata share stable physical ordinal space",
    );
    Ok(())
}

fn verify_metadata(row: &MediaRecord, ordinal: u64, input: &[u8]) -> TestResult {
    assert_eq!(
        row.status,
        Status::Partial,
        "metadata-only rows have no controller coverage"
    );
    let payload = row.data.get("payload").ok_or("metadata payload")?;
    assert_eq!(
        payload
            .get("controller_disposition")
            .and_then(Value::as_str),
        Some("unavailable"),
        "controller disposition"
    );
    assert_eq!(
        payload
            .pointer("/database_name/utf8")
            .and_then(Value::as_str),
        Some("second.caf"),
        "duplicate authored name"
    );
    let value = |field| {
        payload
            .pointer(field)
            .and_then(Value::as_u64)
            .ok_or("metadata range component")
    };
    let start = usize::try_from(value("/metadata_range/start")?)?;
    let end = usize::try_from(value("/metadata_range/end_exclusive")?)?;
    let expected = input.get(start..end).ok_or("source metadata range")?;
    let actual: Vec<u8> = serde_json::from_value(
        payload
            .get("metadata_bytes")
            .ok_or("metadata bytes")?
            .clone(),
    )?;
    assert_eq!(actual, expected, "exact original metadata record");
    let rate = payload
        .pointer("/timing/frames_per_second")
        .and_then(Value::as_u64);
    assert_eq!(
        rate,
        (ordinal == 1).then_some(60),
        "zero rate is retained but unresolved"
    );
    Ok(())
}

fn verify_origins(base: &Path, ordinal: usize) -> TestResult {
    let ordinal = ordinal.to_string();
    let file = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut fields = 0;
    for row in file.lines().map(serde_json::from_str::<Value>) {
        let row = row?;
        if row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation")
            || row.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
        {
            continue;
        }
        let field = row.get("field").and_then(Value::as_str);
        let (encrypted, complete) = match field {
            Some("/data/payload/database_name/utf8" | "/data/payload/metadata_bytes/0") => {
                (Some(true), true)
            }
            Some("/data/payload/controller_disposition") => (None, false),
            _ => continue,
        };
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            encrypted,
            "encrypted origin at {field:?}"
        );
        assert_eq!(
            row.get("sourceSetComplete").and_then(Value::as_bool),
            Some(complete),
            "source coverage at {field:?}"
        );
        fields += 1;
    }
    assert_eq!(
        fields, 3,
        "read metadata and unavailable controller are evidenced independently"
    );
    Ok(())
}
