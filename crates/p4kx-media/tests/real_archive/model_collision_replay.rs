// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::model::{PhysicsBlob, PhysicsGeometry, StatObjPhysics};
use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{TestResult, privacy};

const SOURCE_SAMPLES: usize = 96;
const MAX_ENTRY_BYTES: u64 = 256 << 10;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_collision_slots_match_original_bytes_and_model_media_replay()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let source = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&source);
    watch.check()?;
    let data = select(&source, &watch)?;
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let original_digest = Sha256::digest(&data);
    let expected = inspect(&data)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("sample.cgf", data))
        .write_to(&path)?;
    let replay = Archive::open(path)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } = p4kx_media::export(&replay, &output, &options)?
    else {
        return Err("selected original payload did not produce a complete replay".into());
    };
    let base = output.join(&index.artifact_base);
    let rows = fs::read_to_string(base.join("model.jsonl"))?;
    let actual = check_rows(&base, &rows, &original_digest, &expected.slots)?;
    if expected.tally != actual {
        return Err("replay occurrence counts differ from source collision payloads".into());
    }
    watch.check()?;
    writeln!(io::stdout().lock(), "original collision replay: {actual:?}")?;
    Ok(())
}

fn select(source: &Archive, watch: &ArchiveWatch) -> TestResult<Vec<u8>> {
    let candidates = source
        .entries()
        .iter()
        .filter(|entry| {
            entry.uncompressed_size() <= MAX_ENTRY_BYTES
                && (!entry.is_encrypted() || source.has_key())
                && entry.path().rsplit_once('.').is_some_and(|(_, ext)| {
                    ext.eq_ignore_ascii_case("cgf") || ext.eq_ignore_ascii_case("cga")
                })
        })
        .collect::<Vec<_>>();
    let limit = candidates.len().min(SOURCE_SAMPLES);
    for ordinal in 0..limit {
        watch.poll()?;
        let entry = candidates
            .get(ordinal * candidates.len() / limit)
            .ok_or("selected physical Entry")?;
        let mut bytes = Vec::new();
        source.open_entry_verified(entry)?.read_to_end(&mut bytes)?;
        let tally = inspect(&bytes)?.tally;
        if tally.slots > 0 && tally.vertices > 0 {
            return Ok(bytes);
        }
    }
    Err("no bounded original collision source with positive vertex coverage".into())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Count {
    chunks: usize,
    slots: usize,
    vertices: usize,
    triangles: usize,
}

#[derive(Default)]
struct Observed {
    tally: Count,
    slots: BTreeMap<[u64; 2], OriginalSlot>,
}

struct OriginalSlot {
    packed_key: u32,
    range: Range<u64>,
}

fn inspect(data: &[u8]) -> TestResult<Observed> {
    let mut observed = Observed::default();
    let Ok(file) = ChunkFile::parse(data) else {
        return Ok(observed);
    };
    for (index, chunk) in file.chunks().iter().enumerate() {
        if chunk.chunk_type() != ivo::STAT_OBJ_PHYSICS {
            continue;
        }
        observed.tally.chunks += 1;
        let physics = StatObjPhysics::read_chunk(&file, index)?;
        let bytes = file.chunk_data(index).ok_or("original collision chunk")?;
        for slot in physics.slots() {
            observed.tally.slots += 1;
            let declared = slot.declared_range();
            let start = usize::try_from(declared.start)?;
            let end = usize::try_from(declared.end)?;
            if slot.bytes() != bytes.get(start..end) {
                return Err("source slot differs from original chunk byte slice".into());
            }
            let source = OriginalSlot {
                packed_key: slot.key(),
                range: chunk.offset() + declared.start..chunk.offset() + declared.end,
            };
            observed
                .slots
                .insert([index as u64, slot.ordinal() as u64], source);
            if let PhysicsBlob::Geometry(PhysicsGeometry::Trimesh(mesh)) = slot.blob() {
                observed.tally.vertices += mesh.vertices().len();
                observed.tally.triangles += mesh.triangles().len();
            }
        }
    }
    Ok(observed)
}

fn check_rows(
    base: &Path,
    rows: &str,
    expected: &[u8],
    slots: &BTreeMap<[u64; 2], OriginalSlot>,
) -> TestResult<Count> {
    let mut tally = Count::default();
    let mut raw = None;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for line in rows.lines() {
        let record: MediaRecord = serde_json::from_str(line)?;
        if !schema.is_valid(&serde_json::to_value(&record)?) {
            return Err("model replay record fails the public schema".into());
        }
        let path = &record
            .source
            .raw
            .as_ref()
            .ok_or("replay retained original source")?
            .path;
        if raw.as_ref().is_some_and(|prior| prior != path) {
            return Err("projection does not share its original retained artifact".into());
        }
        raw = Some(path.clone());
        match record.subresource.namespace.as_str() {
            "model-physics-table" => tally.chunks += 1,
            "model-physics-slot" => {
                tally.slots += 1;
                check_slot(&record, slots)?;
            }
            "model-physics-vertex" => tally.vertices += 1,
            "model-physics-triangle" => tally.triangles += 1,
            _ => {}
        }
    }
    let bytes = fs::read(base.join(raw.ok_or("replay output contains no model source")?))?;
    if Sha256::digest(bytes).as_slice() != expected {
        return Err("retained replay bytes differ from original archive Entry".into());
    }
    Ok(tally)
}

fn check_slot(record: &MediaRecord, slots: &BTreeMap<[u64; 2], OriginalSlot>) -> TestResult {
    let physical = <[u64; 2]>::try_from(record.subresource.indices.as_slice())?;
    let slot = slots.get(&physical).ok_or("physical original slot")?;
    let row = record.data.get("payload").ok_or("slot payload")?;
    for (path, expected) in [
        ("/packed_key", u64::from(slot.packed_key)),
        ("/retained_range/start", slot.range.start),
        ("/retained_range/end_exclusive", slot.range.end),
        ("/declared_length", slot.range.end - slot.range.start),
    ] {
        if row.pointer(path).and_then(Value::as_u64) != Some(expected) {
            return Err("wire slot fact differs from the physical original slot".into());
        }
    }
    Ok(())
}
