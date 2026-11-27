// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{TestResult, privacy};

const SAMPLE_LIMIT: usize = 128;
const SAMPLE_ENTRY_BYTES: u64 = 512 << 10;
const WORD_COUNT: usize = 12;
const VERSION: u32 = 0x900;
const NAMESPACE: &str = "model-lod-distance-words";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_lod_distance_words_match_original_bytes_and_media_replay()
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
    let (bytes, expected) = sample(&source, &watch)?;
    let digest = Sha256::digest(&bytes);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("sample.cgf", bytes))
        .write_to(&path)?;
    let replay = Archive::open(path)?;
    let output = folder.path().join("output");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } = p4kx_media::export(&replay, &output, &options)?
    else {
        return Err("original LOD sample replay did not publish".into());
    };
    let base = output.join(&index.artifact_base);
    let rows = BufReader::new(fs::File::open(base.join("model.jsonl"))?);
    let actual = read_words(rows)?;
    if actual.len() != expected.len()
        || actual.iter().any(|(ordinal, actual)| {
            expected
                .get(ordinal)
                .is_none_or(|source| source != &actual.bits)
        })
    {
        return Err("exported LOD words differ from the installed source".into());
    }
    let (_, artifact) = actual.first_key_value().ok_or("projected LOD words")?;
    if Sha256::digest(fs::read(base.join(&artifact.path))?).as_slice() != digest.as_slice() {
        return Err("raw replay source differs from the verified installed Entry".into());
    }
    watch.check()?;
    writeln!(
        io::stdout().lock(),
        "LOD words replay: {} chunks, {} words",
        actual.len(),
        actual.len() * WORD_COUNT
    )?;
    Ok(())
}

struct WordRow {
    bits: Vec<u32>,
    path: String,
}

type Words = BTreeMap<u64, WordRow>;
type SourceWords = BTreeMap<u64, Vec<u32>>;

fn sample(source: &Archive, watch: &ArchiveWatch) -> TestResult<(Vec<u8>, SourceWords)> {
    let entries = source
        .entries()
        .iter()
        .filter(|entry| {
            entry.uncompressed_size() <= SAMPLE_ENTRY_BYTES
                && (!entry.is_encrypted() || source.has_key())
                && entry
                    .path()
                    .rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("cgf"))
        })
        .collect::<Vec<_>>();
    let limit = SAMPLE_LIMIT.min(entries.len());
    for ordinal in 0..limit {
        watch.poll()?;
        let entry = entries
            .get(ordinal * entries.len() / limit)
            .ok_or("original LOD entry")?;
        let mut bytes = Vec::new();
        source.open_entry_verified(entry)?.read_to_end(&mut bytes)?;
        let observed = original_words(&bytes)?;
        if !observed.is_empty() {
            return Ok((bytes, observed));
        }
    }
    Err("no positive bounded LodDistances source".into())
}

fn original_words(bytes: &[u8]) -> TestResult<SourceWords> {
    let Ok(file) = ChunkFile::parse(bytes) else {
        return Ok(BTreeMap::new());
    };
    let mut rows = BTreeMap::new();
    for (ordinal, chunk) in file.chunks().iter().enumerate() {
        if chunk.chunk_type() != ivo::LOD_DISTANCES
            || chunk.version() != VERSION
            || chunk.size() != (WORD_COUNT * size_of::<u32>()) as u64
        {
            continue;
        }
        let original = file
            .chunk_data(ordinal)
            .ok_or("original positioned words")?;
        let (words, _) = original.as_chunks::<4>();
        rows.insert(
            ordinal as u64,
            words.iter().copied().map(u32::from_le_bytes).collect(),
        );
    }
    Ok(rows)
}

fn read_words(rows: impl BufRead) -> TestResult<Words> {
    let mut words = BTreeMap::new();
    for line in rows.lines() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        if row.subresource.namespace != NAMESPACE {
            continue;
        }
        let Some(actual) = row.data.pointer("/payload/words").and_then(Value::as_array) else {
            continue;
        };
        let raw = row.source.raw.ok_or("original retained artifact")?;
        let [ordinal] = row.subresource.indices.as_slice() else {
            return Err("invalid LOD projection coordinate".into());
        };
        let bits = actual
            .iter()
            .map(read_bits)
            .collect::<TestResult<Vec<_>>>()?;
        if words
            .insert(
                *ordinal,
                WordRow {
                    bits,
                    path: raw.path,
                },
            )
            .is_some()
        {
            return Err("duplicate physical LOD row".into());
        }
    }
    Ok(words)
}

fn read_bits(word: &Value) -> TestResult<u32> {
    let bits = u32::try_from(
        word.get("bits")
            .and_then(Value::as_u64)
            .ok_or("original bits")?,
    )?;
    let binary32 = f32::from_bits(bits);
    let observed = word
        .get("finite_binary32")
        .filter(|value| !value.is_null())
        .map(|value| value.to_string().parse::<f32>().map(f32::to_bits))
        .transpose()?;
    if observed != binary32.is_finite().then_some(bits) {
        return Err("binary32 interpretation differs from its original bits".into());
    }
    Ok(bits)
}
