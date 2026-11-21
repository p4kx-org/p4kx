// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, TestModel, mesh_streams, test_key};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

const STREAM: &str = "model.jsonl";
const INDICES: u32 = 0xEECD_C168;

#[test]
fn model_mesh_fields_reconcile_original_ranges_and_encrypted_index_words() -> TestResult {
    let temp = tempfile::tempdir()?;
    let mut mesh = TestModel::cube().skin_chunk();
    mesh.get_mut(176..180)
        .ok_or("original index count")?
        .copy_from_slice(&3_u32.to_le_bytes());
    mesh.get_mut(184..188)
        .ok_or("original page base")?
        .copy_from_slice(&4_u32.to_le_bytes());
    let expected = mesh_streams(&mesh).ok_or("independent fixture stream inventory")?;
    let (index, stream) = expected
        .streams
        .iter()
        .enumerate()
        .find(|(_, stream)| stream.tag == INDICES)
        .ok_or("fixture index stream")?;
    let file = original_file(mesh);
    let directory = ChunkFile::parse(file.as_slice())?;
    let base = directory
        .chunks()
        .first()
        .ok_or("original source chunk")?
        .offset();
    let archive = original_archive(temp.path(), &file)?;
    let out = temp.path().join("out");
    let receipt = support::published(p4kx_media::export(&archive, &out, &options(false))?)?;
    let rows = support::records(&out, &receipt, STREAM)?;
    let generation = out.join(&receipt.artifact_base);
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "closed model row"
        );
    }
    let (_, header) = row(&rows, "model-mesh", &[0])?;
    assert_field(header, "vertices", &json!(expected.vertices));
    assert_field(header, "indices", &json!(expected.indices));
    assert_field(header, "model_bounds_bits", &bounds(&file, base + 24)?);
    assert_field(header, "scaling_bounds_bits", &bounds(&file, base + 48)?);
    assert_field(
        row(&rows, "model-mesh", &[1])?.1,
        "disposition",
        &json!("unsupported-version"),
    );
    let (_, index_row) = row(&rows, "model-mesh-stream", &[0, index as u64])?;
    let start = base + stream.data.start as u64;
    assert_field(
        index_row,
        "payload",
        &range(start, base + stream.data.end as u64),
    );
    assert_field(index_row, "count_basis", &json!("index-count"));
    let original = header.source.raw.as_ref().ok_or("raw retained model")?;
    assert_eq!(
        std::fs::read(generation.join(&original.path))?,
        file,
        "untouched original bytes"
    );
    assert_submesh(&rows, &generation, index, start)
}

fn assert_submesh(rows: &[MediaRecord], generation: &Path, index: usize, start: u64) -> TestResult {
    let (ordinal, submesh) = row(rows, "model-mesh-submesh", &[0, 0])?;
    assert_field(submesh, "first_index", &json!(0));
    assert_field(submesh, "index_count", &json!(3));
    assert_field(submesh, "vertex_page_base", &json!(4));
    assert_field(
        submesh,
        "source_vertices",
        &json!({
            "status": "stored-indices",
            "stream": index,
            "stride_bytes": 2,
            "indices": range(start, start + 6),
        }),
    );
    let uses = field_uses(
        generation,
        ordinal,
        "/data/payload/source_vertices/indices/start",
        0,
        true,
    )?;
    assert!(
        uses.iter().any(|entry| entry.pointer("/location/indices")
            == Some(&json!([start.to_string(), (start + 6).to_string()]))),
        "source-mapping field carries precisely the retained source index words",
    );
    Ok(())
}

#[test]
fn malformed_mesh_stream_header_keeps_original_siblings_in_diagnostics() -> TestResult {
    let temp = tempfile::tempdir()?;
    let mut mesh = TestModel::cube().skin_chunk();
    let ordinal = mesh_streams(&mesh)
        .ok_or("original complete streams")?
        .streams
        .len() as u64;
    mesh.extend(0xDEAD_CAFE_u32.to_le_bytes());
    let file = TestChunkFile::ivo()
        .chunk(ivo::SKIN_2.id(), 0x900, mesh)
        .bytes();
    let archive = original_archive(temp.path(), &file)?;
    let out = temp.path().join("out");
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &out, &options(true))?
    else {
        return Err("malformed source tail must keep its partial disposition".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = std::fs::read_to_string(base.join(STREAM))?
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    let (_, broken) = row(&rows, "model-mesh-stream", &[0, ordinal])?;
    assert_field(broken, "disposition", &json!("malformed-header"));
    assert_eq!(broken.status, Status::Partial, "partial stream diagnostic");
    assert_eq!(
        row(&rows, "model-mesh", &[0])?.1.status,
        Status::Complete,
        "complete header"
    );
    assert_eq!(
        row(&rows, "model-mesh-stream", &[0, 0])?.1.status,
        Status::Complete,
        "known sibling extent"
    );
    let (_, submesh) = row(&rows, "model-mesh-submesh", &[0, 0])?;
    assert_field(
        submesh,
        "source_vertices",
        &json!({"status": "invalid-index-stream"}),
    );
    assert!(
        !out.join("media-index.json").exists(),
        "partial rows do not publish a complete index"
    );
    Ok(())
}

#[test]
fn opaque_mesh_tail_preserves_its_range_without_masking_known_indices() -> TestResult {
    let temp = tempfile::tempdir()?;
    let mut mesh = TestModel::cube().skin_chunk();
    let ordinal = mesh_streams(&mesh)
        .ok_or("original complete streams")?
        .streams
        .len() as u64;
    let tail_at = mesh.len() as u64;
    mesh.extend(0xDEAD_CAFE_u32.to_le_bytes());
    mesh.extend(u32::MAX.to_le_bytes());
    let file = TestChunkFile::ivo()
        .chunk(ivo::SKIN_2.id(), 0x900, mesh)
        .bytes();
    let directory = ChunkFile::parse(file.as_slice())?;
    let start = directory.chunks().first().ok_or("original chunk")?.offset() + tail_at;
    let archive = original_archive(temp.path(), &file)?;
    let out = temp.path().join("out");
    let receipt = support::published(p4kx_media::export(&archive, &out, &options(false))?)?;
    let rows = support::records(&out, &receipt, STREAM)?;
    let (_, tail) = row(&rows, "model-mesh-stream", &[0, ordinal])?;
    assert_eq!(tail.status, Status::Complete, "opaque bytes were retained");
    assert_field(tail, "disposition", &json!("opaque-tail"));
    assert_field(tail, "payload", &Value::Null);
    assert_field(tail, "retained_tail", &range(start, start + 8));
    let (_, physical) = row(&rows, "model-mesh-submesh", &[0, 0])?;
    assert_eq!(
        physical.data.pointer("/payload/source_vertices/status"),
        Some(&json!("stored-indices")),
        "known prior index stream stays addressable"
    );
    Ok(())
}

fn original_file(mesh: Vec<u8>) -> Vec<u8> {
    TestChunkFile::ivo()
        .chunk(ivo::SKIN_2.id(), 0x900, mesh)
        .chunk(ivo::SKIN.id(), 0x901, b"synthetic opaque layout")
        .bytes()
}

fn original_archive(path: &Path, file: &[u8]) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = path.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Synthetic/source.skinm", file).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn options(diagnostic: bool) -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Model],
        diagnostic,
        ..ExportOptions::default()
    }
}

fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> Result<(usize, &'a MediaRecord), Box<dyn std::error::Error>> {
    let mut matches = rows.iter().enumerate().filter(|(_, row)| {
        row.subresource.namespace == namespace && row.subresource.indices == indices
    });
    let row = matches
        .next()
        .ok_or_else(|| format!("missing {namespace} {indices:?}"))?;
    assert!(matches.next().is_none(), "one row per physical coordinate");
    Ok(row)
}

fn assert_field(row: &MediaRecord, field: &str, expected: &Value) {
    assert_eq!(
        row.data
            .get("payload")
            .and_then(|payload| payload.get(field)),
        Some(expected),
        "exact original {field}"
    );
}

fn range(start: u64, end: u64) -> Value {
    json!({"basis": {"type": "entry"}, "start": start, "end_exclusive": end})
}

fn bounds(file: &[u8], start: u64) -> Result<Value, Box<dyn std::error::Error>> {
    let start = usize::try_from(start)?;
    let stored = file.get(start..start + 24).ok_or("original bounding box")?;
    let words = stored
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(u32::from_le_bytes)
        .collect::<Vec<_>>();
    Ok(json!([
        words.get(..3).ok_or("original min")?,
        words.get(3..).ok_or("original max")?
    ]))
}
