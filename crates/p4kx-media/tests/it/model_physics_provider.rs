// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::support::{self, TestResult};

const STREAM: &str = "model.jsonl";

#[test]
fn typed_collision_rows_preserve_physical_occurrences_and_original_words() -> TestResult {
    let temp = tempfile::tempdir()?;
    let sphere = collision_shape(4, &[1.0, -0.0, 3.0, 4.0]);
    let mesh = triangle();
    let sdf = 0xDEAD_BEB5_u32.to_le_bytes();
    let slots = [
        (0x0002_FFFF, sphere.as_slice()),
        (0x0002_FFFF, &sdf),
        (0x0006_0123, &mesh),
    ];
    let data = container(&physics_table(&slots)?);
    let original = ChunkFile::parse(data.as_slice())?;
    let chunk_start = original
        .chunks()
        .first()
        .ok_or("first authored chunk")?
        .offset();
    let path = temp.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("synthetic.cgf", data).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = temp.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, STREAM)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(schema.is_valid(&serde_json::to_value(row)?));
        assert_eq!(row.status, Status::Complete);
        assert_eq!(row.source.raw.as_ref(), rows[0].source.raw.as_ref());
    }
    let sphere_ordinal = check_rows(&rows, chunk_start)?;
    let base = output.join(&index.artifact_base);
    let word = chunk_start + 104 + 76;
    assert_encrypted_word(
        &base,
        sphere_ordinal,
        "/data/payload/blob/geometry/centre/1/bits",
        word,
    )?;
    Ok(())
}

fn check_rows(rows: &[MediaRecord], chunk_start: u64) -> Result<usize, Box<dyn std::error::Error>> {
    let table = find(rows, "model-physics-table", &[0])?;
    assert_value(&table.data, "/payload/slot_count", &json!(3));
    let sphere = find(rows, "model-physics-slot", &[0, 0])?;
    let sdf = find(rows, "model-physics-slot", &[0, 1])?;
    let key = &json!(0x0002_FFFF);
    assert_value(&sphere.data, "/payload/packed_key", key);
    assert_value(&sdf.data, "/payload/packed_key", key);
    assert_value(&sphere.data, "/payload/node", &Value::Null);
    assert_value(
        &sdf.data,
        "/payload/blob/disposition",
        &json!("signed-distance-field"),
    );
    assert_value(
        &sphere.data,
        "/payload/blob/frame",
        &json!("source-node-z-up-metres"),
    );
    assert_value(
        &sphere.data,
        "/payload/blob/geometry/centre/1/bits",
        &json!(0x8000_0000_u32),
    );
    assert_value(
        &sphere.data,
        "/payload/retained_range/start",
        &json!(chunk_start + 104),
    );
    let vertex = find(rows, "model-physics-vertex", &[0, 2, 1])?;
    assert_value(&vertex.data, "/payload/position/0/value", &json!(1.0));
    let triangle = find(rows, "model-physics-triangle", &[0, 2, 0])?;
    assert_value(&triangle.data, "/payload/indices", &json!([0, 1, 2]));
    assert_value(&triangle.data, "/payload/material_id", &json!(7));
    rows.iter()
        .position(|row| std::ptr::eq(row, sphere))
        .ok_or_else(|| "slot ordinal".into())
}

fn assert_value(value: &Value, path: &str, expected: &Value) {
    assert_eq!(
        value.pointer(path),
        Some(expected),
        "exact wire value at {path}"
    );
}

fn assert_encrypted_word(
    base: &std::path::Path,
    ordinal: usize,
    field: &str,
    word: u64,
) -> TestResult {
    let ordinal = ordinal.to_string();
    let rows = std::fs::read_to_string(base.join("field-origins.jsonl"))?;
    let field = rows
        .lines()
        .map(serde_json::from_str::<Value>)
        .find(|row| {
            row.as_ref().is_ok_and(|row| {
                row.get("field").and_then(Value::as_str) == Some(field)
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("model")
            })
        })
        .ok_or("exact collision word origin")??;
    assert_value(&field, "/containsEncryptedSource", &json!(true));
    assert_value(&field, "/sourceSetComplete", &json!(true));
    let reference = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .and_then(|uses| uses.first())
        .ok_or("source use")?;
    let uses = std::fs::read_to_string(base.join("origin-uses.jsonl"))?;
    let usage = uses
        .lines()
        .map(serde_json::from_str::<Value>)
        .find(|row| {
            row.as_ref()
                .is_ok_and(|row| row.get("id") == Some(reference))
        })
        .ok_or("closed collision use")??;
    assert_value(
        &usage,
        "/location/indices",
        &json!([word.to_string(), (word + 4).to_string()]),
    );
    assert_value(&usage, "/location/field", &json!("physicsFloatWord"));
    Ok(())
}

#[test]
fn malformed_collision_blob_keeps_good_siblings_in_diagnostics() -> TestResult {
    let temp = tempfile::tempdir()?;
    let bad = collision_shape(4, &[f32::NAN, 0.0, 0.0, 1.0]);
    let good = collision_shape(4, &[0.0, 0.0, 0.0, 1.0]);
    let data = physics_table(&[(0, &bad), (1, &good)])?;
    let file = TestChunkFile::ivo()
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x901, &data[..])
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x900, &data[..])
        .bytes();
    let archive = support::archive_with(temp.path(), [TestEntry::stored("synthetic.cgf", file)])?;
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
        return Err("malformed blob must not become a complete generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = std::fs::read_to_string(base.join(STREAM))?
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    let unsupported = find(&rows, "model-physics-table", &[0])?;
    assert_eq!(
        unsupported.data["payload"]["disposition"],
        "unsupported-version"
    );
    assert_eq!(unsupported.status, Status::Complete);
    assert_eq!(
        find(&rows, "model-physics-slot", &[1, 0])?.status,
        Status::Partial
    );
    assert_eq!(
        find(&rows, "model-physics-slot", &[1, 1])?.status,
        Status::Complete
    );
    assert!(!output.join("media-index.json").exists());
    Ok(())
}

fn find<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> Result<&'a MediaRecord, Box<dyn std::error::Error>> {
    rows.iter()
        .find(|row| row.subresource.namespace == namespace && row.subresource.indices == indices)
        .ok_or_else(|| format!("missing {namespace} {indices:?}").into())
}

pub(super) fn collision_shape(kind: u32, words: &[f32]) -> Vec<u8> {
    let mut bytes = 2_u32.to_le_bytes().to_vec();
    bytes.resize(68, 0);
    bytes.extend(kind.to_le_bytes());
    bytes.extend(words.iter().flat_map(|value| value.to_le_bytes()));
    bytes
}

fn triangle() -> Vec<u8> {
    let mut bytes = collision_shape(1, &[]);
    for word in [3_u32, 1, 3, 0, 1] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([1, 0]);
    bytes.extend(15_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.push(7);
    for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bytes.extend(value.to_le_bytes());
    }
    for value in [0_u16, 1, 2] {
        bytes.extend(value.to_le_bytes());
    }
    bytes
}

pub(super) fn physics_table(slots: &[(u32, &[u8])]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = vec![0; 80];
    bytes
        .get_mut(40..44)
        .ok_or("fixture slot count word")?
        .copy_from_slice(&u32::try_from(slots.len())?.to_le_bytes());
    for (key, payload) in slots {
        bytes.extend(u32::try_from(payload.len())?.to_le_bytes());
        bytes.extend(key.to_le_bytes());
    }
    for (_, payload) in slots {
        bytes.extend_from_slice(payload);
    }
    Ok(bytes)
}

fn container(data: &[u8]) -> Vec<u8> {
    TestChunkFile::ivo()
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x900, data)
        .bytes()
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    }
}
