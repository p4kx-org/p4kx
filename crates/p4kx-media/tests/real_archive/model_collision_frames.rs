// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::model::StatObjPhysics;
use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::{TestResult, privacy};

const MAX_SOURCE_BYTES: u64 = 512 << 10;
const SAMPLE_LIMIT: usize = 320;
const RAW_HEADER_BYTES: usize = 32;
const RAW_NODE_BYTES: usize = 208;
type Matrix = [[f64; 4]; 3];
type ExpectedFrames = BTreeMap<[u64; 2], Matrix>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_collision_frames_follow_original_unfiltered_node_words()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let original = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&original);
    watch.check()?;
    let (bytes, expected) = source_sample(&original, &watch)?;
    let dir = tempfile::tempdir()?;
    let replay_file = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("positioned.cgf", bytes))
        .write_to(&replay_file)?;
    let replay = Archive::open(replay_file)?;
    let output = dir.path().join("export");
    let settings = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } = p4kx_media::export(&replay, &output, &settings)?
    else {
        return Err("bounded source replay was incomplete".into());
    };
    let rows = fs::read_to_string(output.join(&index.artifact_base).join("model.jsonl"))?;
    compare_rows(&rows, &expected)?;
    watch.check()?;
    Ok(())
}

fn compare_rows(rows: &str, expected: &ExpectedFrames) -> TestResult {
    let mut checked = BTreeSet::new();
    for line in rows.lines() {
        let row: MediaRecord = serde_json::from_str(line)?;
        if row.subresource.namespace != "model-physics-frame" {
            continue;
        }
        let id = <[u64; 2]>::try_from(row.subresource.indices.as_slice())?;
        let Some(wanted) = expected.get(&id) else {
            continue;
        };
        let entity = row
            .data
            .pointer("/payload/entity")
            .ok_or("exported entity matrix")?;
        let actual: Matrix = serde_json::from_value(entity.clone())?;
        for (actual, wanted) in actual.into_iter().flatten().zip(wanted.iter().flatten()) {
            if (actual - wanted).abs() > 2e-5 * (1.0 + wanted.abs()) {
                return Err(
                    "exported frame differs from independent original matrix arithmetic".into(),
                );
            }
        }
        if row
            .data
            .pointer("/payload/disposition")
            .and_then(Value::as_str)
            != Some("resolved")
            || !checked.insert(id)
        {
            return Err("expected precisely one source frame for the slot".into());
        }
    }
    if checked.len() != expected.len() {
        return Err("original node-bound slots are missing".into());
    }
    Ok(())
}

fn source_sample(archive: &Archive, watch: &ArchiveWatch) -> TestResult<(Vec<u8>, ExpectedFrames)> {
    let candidates: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry.uncompressed_size() <= MAX_SOURCE_BYTES
                && (!entry.is_encrypted() || archive.has_key())
                && entry.path().rsplit_once('.').is_some_and(|(_, ext)| {
                    ext.eq_ignore_ascii_case("cgf") || ext.eq_ignore_ascii_case("cga")
                })
        })
        .collect();
    let sample = candidates.len().min(SAMPLE_LIMIT);
    for at in 0..sample {
        watch.poll()?;
        let entry = candidates
            .get(at * candidates.len() / sample)
            .ok_or("source entry")?;
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        let expected = inspect(&bytes)?;
        if !expected.is_empty() {
            return Ok((bytes, expected));
        }
    }
    Err("bounded sample produced no nontrivial node-bound collision frame".into())
}

fn inspect(bytes: &[u8]) -> TestResult<ExpectedFrames> {
    let mut expected = ExpectedFrames::new();
    let Ok(chunks) = ChunkFile::parse(bytes) else {
        return Ok(expected);
    };
    let nmc = chunks
        .chunks_of(ivo::NODE_MESH_COMBO)
        .filter(|(_, row)| row.version() == 0x900)
        .collect::<Vec<_>>();
    let [(index, _)] = nmc.as_slice() else {
        return Ok(expected);
    };
    let node_source = chunks.chunk_data(*index).ok_or("source node metadata")?;
    let count = usize::try_from(u32::from_le_bytes(original(node_source, 4)?))?;
    let source = node_source
        .get(RAW_HEADER_BYTES..RAW_HEADER_BYTES + count * RAW_NODE_BYTES)
        .ok_or("bounded original node table")?;
    let (nodes, _) = source.as_chunks::<RAW_NODE_BYTES>();
    for (ordinal, _) in chunks.chunks_of(ivo::STAT_OBJ_PHYSICS) {
        let Ok(table) = StatObjPhysics::read_chunk(&chunks, ordinal) else {
            continue;
        };
        for slot in table.slots() {
            let Some(index) = slot.node().map(usize::from) else {
                continue;
            };
            if !matches!(
                slot.blob(),
                p4kx_extraction::model::PhysicsBlob::Geometry(_)
            ) {
                continue;
            }
            let Some(node) = nodes.get(index) else {
                continue;
            };
            if u16::from_le_bytes(original(node, 148)?) == u16::MAX {
                continue;
            }
            if let Ok(matrix) = compose(nodes, index) {
                expected.insert([ordinal as u64, slot.ordinal() as u64], matrix);
            }
        }
    }
    Ok(expected)
}

fn compose(nodes: &[[u8; RAW_NODE_BYTES]], mut index: usize) -> TestResult<Matrix> {
    let mut seen = BTreeSet::new();
    let mut accumulated = identity();
    loop {
        if !seen.insert(index) {
            return Err("cyclic original chain".into());
        }
        let row = nodes.get(index).ok_or("parent row")?;
        accumulated = multiply(local(row)?, accumulated);
        let parent = u16::from_le_bytes(original(row, 148)?);
        if parent == u16::MAX {
            return Ok(accumulated);
        }
        index = usize::from(parent);
    }
}

fn local(source: &[u8]) -> TestResult<Matrix> {
    let mut matrix = [[0.0; 4]; 3];
    for (index, word) in matrix.iter_mut().flatten().enumerate() {
        *word = f64::from(f32::from_le_bytes(original(source, 80 + 4 * index)?));
    }
    if matrix.iter().flatten().all(|word| *word == 0.0) {
        matrix = identity();
    }
    let mut scale = [0.0; 3];
    for (index, value) in scale.iter_mut().enumerate() {
        *value = f64::from(f32::from_le_bytes(original(source, 128 + 4 * index)?));
    }
    if scale.iter().any(|value| *value != 0.0) {
        for row in &mut matrix {
            for (value, factor) in row.iter_mut().zip(scale) {
                *value *= factor;
            }
        }
    }
    Ok(matrix)
}

const fn identity() -> Matrix {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

fn multiply(a: Matrix, b: Matrix) -> Matrix {
    let mut output = [[0.0; 4]; 3];
    for (target, [x, y, z, shift]) in output.iter_mut().zip(a) {
        let [first, second, third] = b;
        for (((at, first), second), third) in target.iter_mut().zip(first).zip(second).zip(third) {
            *at = x.mul_add(first, y.mul_add(second, z * third));
        }
        if let Some(last) = target.last_mut() {
            *last += shift;
        }
    }
    output
}

fn original<const N: usize>(bytes: &[u8], offset: usize) -> TestResult<[u8; N]> {
    Ok(bytes
        .get(offset..offset + N)
        .ok_or("source field")?
        .try_into()?)
}
