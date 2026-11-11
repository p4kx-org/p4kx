// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{ByteOrder, Codec, Wem};
use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use super::{TestResult, audio_export as export, jsonl_rows as rows, wem_conversion as conversion};

const TONE: &str = "audio/tone.wem";
const UNSUPPORTED: &str = "audio/compressed.wem";
const RATE: u32 = 22_050;
const XMA2_FORMAT_TAG: u16 = 0x0166;
const SAMPLES: [i16; 4] = [0, -123, 456, i16::MAX];
const INDEX: &str = "media-index.json";
const ARTIFACT_DIGEST: &str = "/data/payload/outcome/payload/artifact/digest/hex";

#[test]
fn media_cli_wav_is_explicit_sourced_and_atomic() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    let original = TestWem::pcm(TestCodec::Pcm, 1, RATE, &SAMPLES).bytes();
    let xma2 = TestWem::encoded(TestCodec::Tag(XMA2_FORMAT_TAG), 1, RATE, 8).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(TONE, original.as_slice()).encrypted())
        .entry(TestEntry::stored(UNSUPPORTED, xma2))
        .write_to(&source)?;

    let raw = folder.path().join("raw");
    export(&source, &raw, TONE)?.assert().code(0);
    let default_index = fs::read(raw.join(INDEX))?;
    let default: p4kx_media::Index = serde_json::from_slice(&default_index)?;
    let default_rows = rows(&raw.join(default.artifact_base).join("audio.jsonl"))?;
    assert_eq!(
        default_rows.iter().filter(|row| conversion(row)).count(),
        0,
        "omitting the format only retains raw audio"
    );
    export(&source, &raw, TONE)?
        .args(["--audio-format", "raw", "--conflict", "overwrite"])
        .assert()
        .code(0);
    assert_eq!(
        fs::read(raw.join(INDEX))?,
        default_index,
        "explicit raw has the same output as the default"
    );

    let output = folder.path().join("wav");
    export(&source, &output, TONE)?
        .args(["--audio-format", "wav"])
        .assert()
        .code(0);
    verify_wav_and_failed_replacement(&source, &output, &original)
}

fn verify_wav_and_failed_replacement(source: &Path, output: &Path, original: &[u8]) -> TestResult {
    let prior = fs::read(output.join(INDEX))?;
    let index: p4kx_media::Index = serde_json::from_slice(&prior)?;
    let directory = output.join(index.artifact_base);
    let actual = rows(&directory.join("audio.jsonl"))?;
    let mut conversions = actual.iter().enumerate().filter(|(_, row)| conversion(row));
    let (ordinal, mapping) = conversions.next().ok_or("explicit WAV mapping")?;
    assert!(
        conversions.next().is_none(),
        "exactly one physical WEM is converted"
    );
    for (path, expected) in [
        ("/data/payload/requested_format", "wav"),
        ("/data/payload/source_codec", "pcm"),
        ("/data/payload/outcome/type", "written"),
        ("/data/payload/outcome/payload/actual_format", "wav"),
    ] {
        assert_eq!(
            mapping.pointer(path).and_then(Value::as_str),
            Some(expected),
            "{path}"
        );
    }
    let artifact = directory.join(
        mapping
            .pointer("/data/payload/outcome/payload/artifact/path")
            .and_then(Value::as_str)
            .ok_or("placed WAV path")?,
    );
    let bytes = fs::read(&artifact)?;
    let wav = Wem::parse(bytes.as_slice())?;
    let samples: Vec<u8> = SAMPLES.into_iter().flat_map(i16::to_le_bytes).collect();
    assert_eq!(
        (wav.codec(), wav.byte_order(), wav.sample_rate()),
        (Codec::Pcm, ByteOrder::Little, RATE),
        "canonical WAV declaration"
    );
    assert_eq!(wav.data(), samples, "CLI produced exact sample bytes");
    verify_origin(&directory, ordinal, original)?;

    verify_failure(source, output)?;
    assert_eq!(
        fs::read(output.join(INDEX))?,
        prior,
        "both failed requests keep the complete index"
    );
    assert_eq!(
        fs::read(artifact)?,
        bytes,
        "both failed requests keep the original WAV bytes"
    );
    Ok(())
}

fn verify_origin(directory: &Path, ordinal: usize, original: &[u8]) -> TestResult {
    let fields = rows(&directory.join("field-origins.jsonl"))?;
    let ordinal = ordinal.to_string();
    let field = fields
        .iter()
        .find(|field| {
            field.get("field").and_then(Value::as_str) == Some(ARTIFACT_DIGEST)
                && field.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("audio")
                && field.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
        })
        .ok_or("immediate WAV digest receipt")?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(true)),
        "digest includes the selected encrypted input"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(true)),
        "all contributing sources are observed"
    );
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("actual source-use refs")?;

    let wem = Wem::parse(original)?;
    let start = wem.metadata().data_chunk().body_offset;
    let end = start + u64::try_from(wem.data().len())?;
    let location = json!({
        "type": "subresource",
        "namespace": "wemPcmFrames",
        "field": "originalSamples",
        "indices": [start.to_string(), end.to_string()]
    });
    let uses = rows(&directory.join("origin-uses.jsonl"))?;
    let sample_use = uses
        .iter()
        .find(|source_use| {
            source_use.get("id").is_some_and(|id| refs.contains(id))
                && source_use.get("role").and_then(Value::as_str) == Some("conversion")
                && source_use.get("location") == Some(&location)
        })
        .ok_or("exact original PCM byte range")?;
    let sources = rows(&directory.join("origin-sources.jsonl"))?;
    let source = sources
        .iter()
        .find(|row| row.get("id") == sample_use.get("source"))
        .ok_or("physical encrypted WAV input")?;
    assert_eq!(
        source.get("entryIndex"),
        Some(&json!(0)),
        "original sample bytes use the selected physical Entry"
    );
    Ok(())
}

fn verify_failure(source: &Path, output: &Path) -> TestResult {
    let arguments = ["--audio-format", "wav", "--conflict", "overwrite"];
    export(source, output, UNSUPPORTED)?
        .args(arguments)
        .assert()
        .code(1);
    assert!(
        !output.join("diagnostics").exists(),
        "normal refusal creates no diagnostic generation"
    );
    let diagnostic = export(source, output, UNSUPPORTED)?
        .args(arguments)
        .arg("--diagnostic")
        .assert()
        .code(1);
    let report: Value = serde_json::from_slice(&diagnostic.get_output().stdout)?;
    assert_eq!(
        report.get("status"),
        Some(&json!("incomplete")),
        "diagnostic request reports failure"
    );
    let generation = report
        .pointer("/index/generation")
        .and_then(Value::as_str)
        .ok_or("diagnostic generation identifier")?;
    let directory = output.join("diagnostics").join(generation);
    let index: p4kx_media::Index = serde_json::from_slice(&fs::read(directory.join(INDEX))?)?;
    assert!(
        !index.complete,
        "failed requests write only an incomplete diagnostic index"
    );
    assert_eq!(
        index.artifact_base, ".",
        "diagnostic artifact paths are relative to its index"
    );
    let actual = rows(&directory.join(index.artifact_base).join("audio.jsonl"))?;
    let failure = actual
        .iter()
        .find(|row| conversion(row))
        .ok_or("diagnostic conversion mapping")?;
    assert_eq!(
        failure.pointer("/data/payload/outcome/type"),
        Some(&json!("failed")),
        "failed codec has a typed outcome"
    );
    assert_eq!(
        failure.pointer("/data/payload/outcome/payload/type"),
        Some(&json!("unsupported-codec")),
        "original codec refusal is durable"
    );
    Ok(())
}
