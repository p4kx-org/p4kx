// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_audio::Wem;
use p4kx_extraction::{
    Archive, AudioConversion, Checksum, ConflictPolicy, ExtractOptions, extract,
};
use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem, test_signal};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

const PCM_ENTRY: &str = "Data\\Sounds\\known.wem";
const WAV: &str = "Data/Sounds/known.wav";
const RECEIPT: &str = "Data/Sounds/known.wav.p4kx-origin.jsonl";

#[test]
fn bulk_wav_addresses_original_encrypted_samples_and_binds_the_unchanged_payload() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let bytes = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(16, 1)).bytes();
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(PCM_ENTRY, bytes.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let selected: Vec<_> = archive.entries().iter().collect();
    let target = dir.path().join("out");
    fs::create_dir(&target)?;
    let options = ExtractOptions {
        convert: true,
        audio: AudioConversion::Ogg,
        policy: ConflictPolicy::Skip,
        ..ExtractOptions::default()
    };
    let report = extract(&archive, &selected, &target, &options);
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        report.converted(),
        Some(1),
        "same fallback to WAV and count"
    );
    let [artifact] = report.sound_artifacts() else {
        return Err("one converted WAV and companion".into());
    };
    assert_eq!(artifact.path, WAV, "target-relative path");
    assert_eq!(artifact.origin_receipt, RECEIPT, "portable receipt path");

    let written = fs::read(target.join(WAV))?;
    let wem = Wem::parse(&bytes)?;
    let mut ordinary = Vec::new();
    let ordinary_report = wem.write_wav(&mut ordinary)?;
    assert_eq!(written, ordinary, "the existing WAV writer's bytes");
    let sha = Checksum::Sha256(Sha256::digest(&written).into()).hex();
    assert_eq!(
        artifact.digest.byte_length(),
        ordinary_report.size,
        "byte-bound summary"
    );
    assert_eq!(
        artifact.digest.sha256(),
        sha,
        "digest of the installed waveform"
    );

    let receipt = fs::read(target.join(RECEIPT))?;
    let rows = read_companion(&receipt)?;
    let header = rows.first().ok_or("origin binding")?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(sha)),
        "portable byte binding"
    );
    assert_sample_origins(&rows, &wem, ordinary_report.size)?;
    check_keep(&archive, &selected, &target, options, &receipt)
}

fn assert_sample_origins(
    rows: &[Value],
    wem: &Wem<impl AsRef<[u8]>>,
    output_size: u64,
) -> TestResult {
    let samples = field(rows, "pcmSamples")?;
    assert_eq!(
        samples.pointer("/address/start"),
        Some(&json!("44")),
        "mono PCM payload start"
    );
    assert_eq!(
        samples.pointer("/address/endExclusive"),
        Some(&json!(output_size.to_string())),
        "the sample range reaches the installed payload end"
    );
    assert_eq!(
        samples.get("containsEncryptedSource"),
        Some(&json!(true)),
        "encrypted original sample bytes"
    );
    assert_eq!(
        samples.get("sourceSetComplete"),
        Some(&json!(true)),
        "all actual sample sources are known"
    );
    assert!(
        field_uses(rows, samples)?.iter().any(|used| {
            let start = wem.info_source().data_body_offset();
            let end = start + wem.data().len() as u64;
            used.get("role") == Some(&json!("conversion"))
                && used.pointer("/location/field") == Some(&json!("decodedAudio"))
                && used.pointer("/location/indices")
                    == Some(&json!([start.to_string(), end.to_string()]))
        }),
        "PCM range resolves to the exact original WEM DATA bytes"
    );
    assert_eq!(
        field(rows, "bitsPerSample")?.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the fixed output format field is tool-authored"
    );
    Ok(())
}

fn check_keep(
    archive: &Archive,
    entries: &[&p4kx_extraction::Entry],
    target: &Path,
    options: ExtractOptions<'_>,
    receipt: &[u8],
) -> TestResult {
    let keep = extract(archive, entries, target, &options);
    assert!(keep.is_complete(), "{keep:?}");
    assert_eq!(keep.skipped(), &[PCM_ENTRY], "Skip retains the output");
    assert!(
        keep.sound_artifacts().is_empty(),
        "no newly attributed bytes"
    );
    assert_eq!(
        fs::read(target.join(RECEIPT))?,
        receipt,
        "keep the artifact's original companion"
    );

    let compare = ExtractOptions {
        policy: ConflictPolicy::IfChanged,
        ..options
    };
    let keep = extract(archive, entries, target, &compare);
    assert!(keep.is_complete(), "{keep:?}");
    assert_eq!(
        keep.skipped(),
        &[PCM_ENTRY],
        "IfChanged retains identical output"
    );
    assert!(
        keep.sound_artifacts().is_empty(),
        "IfChanged writes no new sound"
    );
    assert_eq!(
        fs::read(target.join(RECEIPT))?,
        receipt,
        "keep the artifact's original companion"
    );
    Ok(())
}

fn read_companion(bytes: &[u8]) -> Result<Vec<Value>, Box<dyn Error>> {
    let rows: Vec<Value> = std::str::from_utf8(bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let schema = schemars::schema_for!(p4kx_extraction::OriginCompanionRow<'static>).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    for row in &rows {
        validator.validate(row).map_err(|error| error.to_string())?;
    }
    Ok(rows)
}

fn field<'rows>(rows: &'rows [Value], name: &str) -> Result<&'rows Value, Box<dyn Error>> {
    rows.iter()
        .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing WAV origin for {name}").into())
}

fn field_uses<'rows>(
    rows: &'rows [Value],
    field: &Value,
) -> Result<Vec<&'rows Value>, Box<dyn Error>> {
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("dependencies")?;
    references
        .iter()
        .map(|id| {
            rows.iter()
                .filter_map(|row| row.get("sourceUse"))
                .find(|used| used.get("id") == Some(id))
                .ok_or_else(|| "unresolved source use".into())
        })
        .collect()
}
