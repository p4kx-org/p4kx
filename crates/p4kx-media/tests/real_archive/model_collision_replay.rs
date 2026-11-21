// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Build};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{TestResult, privacy};

#[path = "model_collision_table_oracle.rs"]
mod oracle;
#[path = "model_collision_replay_tests.rs"]
mod synthetic;

use oracle::{Evidence, Tally};

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
    let build = source.build().ok_or("authoritative installed Build")?;
    let directory = source
        .path()
        .parent()
        .ok_or("installed channel directory")?;
    let mut source_tally = Tally::default();
    for (ordinal, sample) in select(&source, &watch)?.iter().enumerate() {
        let expected = Evidence::read(sample)?;
        replay(sample, expected.partial(), |generation, rows| {
            check_rows(generation, rows, &Sha256::digest(sample), &expected)?;
            accumulate(&mut source_tally, expected.tally());
            Ok(())
        })?;
        watch.check()?;
        writeln!(
            io::stdout().lock(),
            "Reconciled original collision sample {}.",
            ordinal + 1
        )?;
    }
    if Build::read_from(directory)?.as_ref() != Some(build) {
        return Err("installed Build changed during collision replay".into());
    }
    writeln!(
        io::stdout().lock(),
        "Installed change {}, original collision tables {}, dispositions {:?}, vertices {}, triangles {}.",
        build.change_number(),
        source_tally.tables,
        source_tally.dispositions,
        source_tally.vertices,
        source_tally.triangles,
    )?;
    Ok(())
}

fn accumulate(total: &mut Tally, addition: Tally) {
    total.tables += addition.tables;
    total.vertices += addition.vertices;
    total.triangles += addition.triangles;
    for (kind, count) in addition.dispositions {
        *total.dispositions.entry(kind).or_default() += count;
    }
}

fn replay(
    data: &[u8],
    partial_collision: bool,
    check: impl FnOnce(&Path, &str) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("sample.cgf", data))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        diagnostic: true,
        ..ExportOptions::default()
    };
    let generation = match p4kx_media::export(&archive, &output, &options)? {
        ExportOutcome::Published { index, .. } if !partial_collision => {
            output.join(&index.artifact_base)
        }
        ExportOutcome::Incomplete { diagnostic, .. } => {
            diagnostic.parent().ok_or("diagnostic generation")?.into()
        }
        _ => return Err("original sample replay did not produce a Model Media generation".into()),
    };
    let rows = fs::read_to_string(generation.join("model.jsonl"))?;
    check(&generation, &rows)
}

fn select(source: &Archive, watch: &ArchiveWatch) -> TestResult<Vec<Vec<u8>>> {
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
    let mut samples = Vec::new();
    let mut dispositions = BTreeSet::new();
    let limit = candidates.len().min(SOURCE_SAMPLES);
    for ordinal in 0..limit {
        watch.poll()?;
        let entry = candidates
            .get(ordinal * candidates.len() / limit)
            .ok_or("selected physical Entry")?;
        let mut bytes = Vec::new();
        source.open_entry_verified(entry)?.read_to_end(&mut bytes)?;
        let evidence = Evidence::read(&bytes)?;
        let tally = evidence.tally();
        let contributes = tally
            .dispositions
            .keys()
            .any(|kind| !dispositions.contains(kind));
        if contributes {
            dispositions.extend(tally.dispositions.keys().copied());
            samples.push(bytes);
        }
    }
    if !dispositions.contains("trimesh") {
        return Err("no bounded original triangle collision witness".into());
    }
    Ok(samples)
}

#[derive(Default)]
struct Coverage {
    tables: BTreeSet<u64>,
    slots: BTreeSet<[u64; 2]>,
    vertices: BTreeSet<[u64; 3]>,
    triangles: BTreeSet<[u64; 3]>,
}

fn check_rows(generation: &Path, rows: &str, digest: &[u8], expected: &Evidence<'_>) -> TestResult {
    let mut coverage = Coverage::default();
    let mut raw = None;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for line in rows.lines() {
        let record: MediaRecord = serde_json::from_str(line)?;
        if !schema.is_valid(&serde_json::to_value(&record)?) {
            return Err("collision replay row fails the public schema".into());
        }
        let path = &record
            .source
            .raw
            .as_ref()
            .ok_or("retained original source")?
            .path;
        if raw.as_ref().is_some_and(|prior| prior != path) {
            return Err("collision rows use different retained physical Entries".into());
        }
        raw = Some(path.clone());
        coverage.row(&record, expected)?;
    }
    let bytes = fs::read(generation.join(raw.ok_or("replay output contains no Model sources")?))?;
    if Sha256::digest(bytes).as_slice() != digest {
        return Err("retained replay bytes differ from original archive Entry".into());
    }
    let tally = expected.tally();
    if (
        coverage.tables.len(),
        coverage.slots.len(),
        coverage.vertices.len(),
        coverage.triangles.len(),
    ) != (
        expected.tables.len(),
        expected.slots.len(),
        tally.vertices,
        tally.triangles,
    ) {
        return Err("collision replay lost original physical rows".into());
    }
    Ok(())
}

impl Coverage {
    fn row(&mut self, row: &MediaRecord, originals: &Evidence<'_>) -> TestResult {
        let indices = row.subresource.indices.as_slice();
        match row.subresource.namespace.as_str() {
            "model-physics-table" => {
                let [chunk] = <[u64; 1]>::try_from(indices)?;
                unique(&mut self.tables, chunk)?;
                let expected = originals
                    .tables
                    .get(&chunk)
                    .ok_or("original collision table")?;
                compare(row, &expected.value(), expected.partial())
            }
            "model-physics-slot" => {
                let identity = <[u64; 2]>::try_from(indices)?;
                unique(&mut self.slots, identity)?;
                let expected = originals
                    .slots
                    .get(&identity)
                    .ok_or("original collision slot")?;
                compare(row, &expected.value(), expected.partial())
            }
            "model-physics-vertex" => {
                mesh_row(row, originals, &mut self.vertices, MeshRecord::Vertex)
            }
            "model-physics-triangle" => {
                mesh_row(row, originals, &mut self.triangles, MeshRecord::Triangle)
            }
            _ => Ok(()),
        }
    }
}

fn unique<T: Ord>(seen: &mut BTreeSet<T>, identity: T) -> TestResult {
    if !seen.insert(identity) {
        return Err("duplicate physical collision row".into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum MeshRecord {
    Vertex,
    Triangle,
}

fn mesh_row(
    row: &MediaRecord,
    originals: &Evidence<'_>,
    seen: &mut BTreeSet<[u64; 3]>,
    shape: MeshRecord,
) -> TestResult {
    let identity = <[u64; 3]>::try_from(row.subresource.indices.as_slice())?;
    unique(seen, identity)?;
    let [chunk, slot, ordinal] = identity;
    let mesh = originals
        .slots
        .get(&[chunk, slot])
        .and_then(|slot| slot.blob.mesh())
        .ok_or("row must belong to an original triangle mesh slot")?;
    let ordinal = usize::try_from(ordinal)?;
    let value = match shape {
        MeshRecord::Vertex => mesh.vertex(ordinal),
        MeshRecord::Triangle => mesh.triangle(ordinal),
    }
    .ok_or("row has no original mesh element")?;
    compare(row, &value, false)
}

fn compare(row: &MediaRecord, original: &Value, partial: bool) -> TestResult {
    let actual = row.data.get("payload").ok_or("collision row payload")?;
    let status = if partial {
        Status::Partial
    } else {
        Status::Complete
    };
    if row.status != status || !equal(actual, original) {
        return Err(
            "Media collision row differs from the original layout or its disposition".into(),
        );
    }
    Ok(())
}

fn equal(actual: &Value, original: &Value) -> bool {
    match (actual, original) {
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && b.iter()
                    .all(|(key, expected)| a.get(key).is_some_and(|actual| equal(actual, expected)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Value::Number(a), Value::Number(b)) if a.is_f64() && b.is_f64() => {
            let (Some(a), Some(b)) = (a.as_f64(), b.as_f64()) else {
                return false;
            };
            binary32(a).to_bits() == binary32(b).to_bits()
        }
        _ => actual == original,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the original collision scalar is binary32"
)]
const fn binary32(value: f64) -> f32 {
    value as f32
}
