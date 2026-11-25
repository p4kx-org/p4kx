// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_audio::Wem;
use p4kx_extraction::{
    Archive, AudioConversion, Checksum, ConflictPolicy, ConvertedSoundArtifact, Exclusion,
    ExtractOptions, FolderSource, convert_folder, extract,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestOpus, TestOpusFraming, TestVorbis};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::{TestResult, field, field_uses, read_companion};

const ENTRY: &str = "Data\\Sounds\\converted.wem";
const ARCHIVE_WAV: &str = "Data/Sounds/converted.wav";

#[test]
fn archive_wav_binds_vorbis_decode_and_lookahead_without_changing_samples() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("Data.p4k");
    let output = dir.path().join("out");
    fs::create_dir(&output)?;
    let vorbis = TestVorbis::new(2, 48_000, &[false; 40])
        .tone(7)
        .sample_count(38 * 128);
    let bytes = vorbis.wem().smpl_loop(0, 1, 2).bytes();
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd(ENTRY, bytes.as_slice()).encrypted())
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(p4kx_testkit::test_key()))?;
    let selection: Vec<_> = archive.entries().iter().collect();
    let options = options();
    let report = extract(&archive, &selection, &output, &options);
    assert!(report.is_complete(), "{report:?}");
    let [artifact] = report.sound_artifacts() else {
        return Err("one decoded Vorbis artifact".into());
    };
    let wem = Wem::parse(&bytes)?;
    let rows = check_written(artifact, &output, ARCHIVE_WAV, &wem, &json!(true))?;
    check_vorbis_sources(&rows, &wem, &vorbis)?;
    let companion = output.join(&artifact.origin_receipt);
    let original = fs::read(&companion)?;
    let repeated = extract(&archive, &selection, &output, &options);
    assert_eq!(repeated.skipped(), &[ENTRY], "identical WAV retained");
    assert!(
        repeated.sound_artifacts().is_empty(),
        "no new WAV installed"
    );
    assert_eq!(
        fs::read(companion)?,
        original,
        "matching companion retained"
    );
    Ok(())
}

fn check_vorbis_sources(
    rows: &[Value],
    wem: &Wem<impl AsRef<[u8]>>,
    vorbis: &TestVorbis,
) -> TestResult {
    let data = wem.metadata().data_chunk();
    let audio_start = data.body_offset + 2 + vorbis.setup().len() as u64;
    let decoded_end = audio_start
        + vorbis
            .packets()
            .iter()
            .take(39)
            .map(|packet| 2 + packet.len() as u64)
            .sum::<u64>();
    let sample_uses = field_uses(rows, field(rows, "pcmSamples")?)?;
    assert_span(
        &sample_uses,
        "setupPacket",
        data.body_offset..audio_start,
        "conversion",
    );
    assert_span(
        &sample_uses,
        "audioDecoded",
        audio_start..decoded_end,
        "conversion",
    );
    assert_span(
        &sample_uses,
        "audioLookahead",
        decoded_end..data.body_end_exclusive(),
        "selector",
    );
    Ok(())
}

#[test]
fn folder_wav_binds_only_replayed_opus_seek_words_and_packet_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("in");
    let output = dir.path().join("out");
    fs::create_dir(&input)?;
    let fixture = TestOpus::new(TestOpusFraming::Wwise, 6, 9_600).declared_sample_count(1_500);
    let bytes = fixture.wem()?.bytes();
    fs::write(input.join("opus.wem"), &bytes)?;
    let folder = FolderSource::scan(&input, None)?;
    let selection = folder.select(None, &Exclusion::default());
    let report = convert_folder(&folder, &selection, false, &output, &options())?;
    assert!(report.is_complete(), "{report:?}");
    let [artifact] = report.sound_artifacts() else {
        return Err("one decoded Opus artifact".into());
    };
    let wem = Wem::parse(&bytes)?;
    let rows = check_written(artifact, &output, "opus.wav", &wem, &Value::Null)?;
    let encoded = fixture.packets()?;
    let count = (usize::from(encoded.pre_skip) + 1_500).div_ceil(960);
    assert!(
        count < encoded.packets.len(),
        "decoding stops at the trimmed packet prefix"
    );
    let emitted = encoded
        .packets
        .iter()
        .take(count)
        .map(|packet| packet.len() as u64)
        .sum::<u64>();
    let mut source = wem.seekable_source()?;
    let chunks = source.chunks().collect::<Result<Vec<_>, _>>()?;
    let seek = chunks
        .iter()
        .find(|chunk| chunk.id == *b"seek")
        .ok_or("original seek table")?;
    let audio = wem.metadata().data_chunk().body_offset;
    let uses = field_uses(&rows, field(&rows, "pcmSamples")?)?;
    assert_span(&uses, "audioReplayed", audio..audio + emitted, "conversion");
    assert_span(
        &uses,
        "seekTableReplayed",
        seek.body_offset..seek.body_offset + 2 * count as u64,
        "selector",
    );
    Ok(())
}

fn check_written(
    artifact: &ConvertedSoundArtifact,
    target: &Path,
    path: &str,
    wem: &Wem<impl AsRef<[u8]>>,
    encryption: &Value,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    assert_eq!(artifact.path, path, "target-relative WAV");
    assert_eq!(
        artifact.origin_receipt,
        format!("{path}.p4kx-origin.jsonl"),
        "target-relative receipt"
    );
    let bytes = fs::read(target.join(path))?;
    let mut original = Vec::new();
    let report = wem.write_wav(&mut original)?;
    assert_eq!(bytes, original, "same WAV bytes as the buffer decoder");
    let sha = Checksum::Sha256(Sha256::digest(&bytes).into()).hex();
    assert_eq!(
        artifact.digest.byte_length(),
        report.size,
        "exact WAV length"
    );
    assert_eq!(artifact.digest.sha256(), sha, "digest of the committed WAV");
    let rows = read_companion(&fs::read(target.join(&artifact.origin_receipt))?)?;
    let header = rows.first().ok_or("artifact binding")?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(sha)),
        "portable digest"
    );
    let samples = field(&rows, "pcmSamples")?;
    let start = report.size - 2 * u64::from(report.channels) * report.sample_count;
    assert_eq!(
        samples.pointer("/address/start"),
        Some(&json!(start.to_string())),
        "actual PCM offset"
    );
    assert_eq!(
        samples.pointer("/address/endExclusive"),
        Some(&json!(report.size.to_string())),
        "PCM reaches WAV end"
    );
    assert_eq!(
        samples.get("sourceSetComplete"),
        Some(&json!(true)),
        "all sample contributors"
    );
    assert_eq!(
        samples.get("containsEncryptedSource"),
        Some(encryption),
        "source encryption"
    );
    Ok(rows)
}

fn assert_span(uses: &[&Value], name: &str, bytes: Range<u64>, role: &str) {
    let extent = json!([bytes.start.to_string(), bytes.end.to_string()]);
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/field").and_then(Value::as_str) == Some(name)
                && used.get("role").and_then(Value::as_str) == Some(role)
                && used.pointer("/location/indices") == Some(&extent)
        }),
        "original {name} byte range {bytes:?} with {role}: {uses:?}"
    );
}

fn options() -> ExtractOptions<'static> {
    ExtractOptions {
        convert: true,
        audio: AudioConversion::Wav,
        policy: ConflictPolicy::IfChanged,
        ..ExtractOptions::default()
    }
}
