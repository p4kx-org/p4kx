// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Output;

use p4kx_extraction::{OriginDigestWriter, Sound, Wem};
use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use super::{SOUND, TestResult};

#[path = "wav_compressed_receipt.rs"]
mod compressed;

#[test]
fn selective_file_observation_matches_the_wav_companion_supported_codecs() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pcm = directory.path().join("linear.wem");
    let vorbis = directory.path().join("vorbis.wem");
    let unsupported = directory.path().join("unsupported.wem");
    fs::write(&pcm, TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[1]).bytes())?;
    fs::write(
        &vorbis,
        TestWem::encoded(TestCodec::Vorbis, 1, 48_000, 10).bytes(),
    )?;

    fs::write(
        &unsupported,
        TestWem::encoded(TestCodec::Tag(0x1234), 1, 48_000, 10).bytes(),
    )?;
    for source in [&pcm, &vorbis] {
        assert!(
            Sound::from_file_observed_for_wav(source)?
                .source_input()
                .is_some(),
            "supported WAV observes its original source"
        );
    }
    assert!(
        Sound::from_file_observed_for_wav(&unsupported)?
            .source_input()
            .is_none(),
        "unsupported codec does not gather provenance before failing"
    );
    Ok(())
}

#[test]
fn pcm_wav_receipt_rebinds_identical_bytes_to_the_successful_source() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("synthetic.p4k");
    let loose = directory.path().join("synthetic.wem");
    let output = directory.path().join("output.wav");
    let companion = directory.path().join("output.wav.p4kx-origin.jsonl");
    let input = TestWem::pcm(TestCodec::Pcm, 2, 48_000, &[])
        .data([1, 2, 3, 4, 99])
        .bytes();
    let parsed = Wem::parse(input.as_slice())?;
    let source_at = parsed.info_source().data_body_offset();
    fs::write(&loose, &input)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOUND, input.clone()).encrypted())
        .write_to(&archive)?;

    let written = wav(&archive, Some(SOUND), &output)?;
    assert_eq!(written.status.code(), Some(0), "{written:?}");
    let summary: Value = serde_json::from_slice(&written.stdout)?;
    assert_eq!(summary["originReceipt"], "output.wav.p4kx-origin.jsonl");
    let first = read_receipt(&companion)?;
    let samples = field(&first, "pcmSamples")?;
    assert_eq!(samples["containsEncryptedSource"], true);
    assert_eq!(samples["sourceSetComplete"], true);
    assert_eq!(
        samples["address"],
        json!({"type": "wavBytes", "field": "pcmSamples", "start": "44", "endExclusive": "48"})
    );
    assert_eq!(
        evidence(&first, "wemPcmFrames")?,
        vec![json!([source_at.to_string(), (source_at + 4).to_string()])]
    );

    let actual = fs::read(&output)?;
    let mut original = Vec::new();
    parsed.write_wav(&mut original)?;
    assert_eq!(actual, original, "the companion does not change WAV bytes");
    let mut measured = OriginDigestWriter::new(std::io::sink());
    measured.write_all(&actual)?;
    let digest = measured.finish()?;
    assert_eq!(
        first.first().ok_or("header")?["binding"]["sha256"],
        digest.sha256()
    );

    let written = p4kx_testkit::p4kx()?
        .args(["audio", "wav"])
        .arg(loose)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert_eq!(written.status.code(), Some(0), "{written:?}");
    let second = read_receipt(&companion)?;
    let samples = field(&second, "pcmSamples")?;
    assert!(samples["containsEncryptedSource"].is_null());
    assert_eq!(samples["sourceSetComplete"], true);
    assert_eq!(
        fs::read(output)?,
        actual,
        "the replacement bytes are identical"
    );
    Ok(())
}

#[test]
fn ima_wav_receipt_excludes_reserved_bytes_and_keeps_admitted_tail() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ima.wem");
    let output = directory.path().join("output.wav");
    let mut data = vec![0; 36];
    data.extend_from_slice(&[0; 5]);
    let input = TestWem::ima_adpcm(1, 48_000, &[]).data(data).bytes();
    let parsed = Wem::parse(input.as_slice())?;
    let at = parsed.info_source().data_body_offset();
    fs::write(&path, &input)?;
    let result = wav(&path, None, &output)?;
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let body = read_receipt(&directory.path().join("output.wav.p4kx-origin.jsonl"))?;
    let mut uses = evidence(&body, "wemImaAdpcmByteRuns")?;
    let mut expected = vec![
        json!([at.to_string(), "1", "36", "0", "3"]),
        json!([at.to_string(), "1", "36", "4", "32"]),
        json!([(at + 36).to_string(), "1", "5", "0", "3"]),
        json!([(at + 36).to_string(), "1", "5", "4", "1"]),
    ];
    uses.sort_by_key(Value::to_string);
    expected.sort_by_key(Value::to_string);
    assert_eq!(
        uses, expected,
        "only used channel history, index and nibble bytes"
    );
    Ok(())
}

#[test]
fn ptadpcm_wav_receipt_addresses_the_multichannel_header_and_loop() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ptadpcm.wem");
    let output = directory.path().join("output.wav");
    let input = TestWem::ptadpcm(6, 48_000, 36, &[])
        .data(vec![0; 6 * 36 + 5])
        .smpl_loop(0, 1, 2)
        .bytes();
    let parsed = Wem::parse(input.as_slice())?;
    let at = parsed.info_source().data_body_offset();
    fs::write(&path, &input)?;
    let result = wav(&path, None, &output)?;
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let body = read_receipt(&directory.path().join("output.wav.p4kx-origin.jsonl"))?;
    let bytes = fs::read(&output)?;
    for (field_name, start, expected) in [
        ("channelMask", 40, 0x3f_u32),
        ("loopStart", 112, 1),
        ("loopEndInclusive", 116, 2),
    ] {
        let address = &field(&body, field_name)?["address"];
        assert_eq!(address["start"], start.to_string(), "{field_name}");
        assert_eq!(
            address["endExclusive"],
            (start + 4).to_string(),
            "{field_name}"
        );
        assert_eq!(
            bytes.get(start..start + 4),
            Some(expected.to_le_bytes().as_slice()),
            "{field_name}"
        );
    }
    assert_eq!(
        evidence(&body, "wemAdpcmBlocks")?,
        vec![json!([at.to_string(), (at + 6 * 36).to_string()])],
        "PTADPCM drops the incomplete final block"
    );
    Ok(())
}

#[test]
fn failed_adpcm_decode_keeps_the_existing_artifact_and_receipt() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("input.wem");
    let output = directory.path().join("output.wav");
    let companion = directory.path().join("output.wav.p4kx-origin.jsonl");
    fs::write(
        &input,
        TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[1]).bytes(),
    )?;
    let result = wav(&input, None, &output)?;
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let waveform = fs::read(&output)?;
    let previous = fs::read(&companion)?;

    fs::write(
        &input,
        TestWem::ima_adpcm(1, 48_000, &[0; 2])
            .block_align(2)
            .bytes(),
    )?;
    let result = wav(&input, None, &output)?;
    assert!(!result.status.success(), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("block alignment"),
        "the original codec error survives staging: {result:?}"
    );
    assert_eq!(fs::read(output)?, waveform);
    assert_eq!(fs::read(companion)?, previous);
    Ok(())
}

#[test]
fn a_foreign_sibling_receipt_leaves_the_existing_wav_unmodified() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("input.wem");
    let output = directory.path().join("output.wav");
    let companion = directory.path().join("output.wav.p4kx-origin.jsonl");
    fs::write(
        &input,
        TestWem::pcm(TestCodec::Pcm, 1, 48_000, &[1]).bytes(),
    )?;
    fs::write(&output, b"old waveform")?;
    fs::write(&companion, b"unrelated contents")?;

    let result = wav(&input, None, &output)?;
    assert!(!result.status.success(), "{result:?}");
    assert_eq!(fs::read(output)?, b"old waveform");
    assert_eq!(fs::read(companion)?, b"unrelated contents");
    assert!(!directory.path().join(".p4kx-origin.lock").exists());
    Ok(())
}

fn wav(
    input: &Path,
    entry: Option<&str>,
    output: &Path,
) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(p4kx_testkit::p4kx()?
        .args(["--json", "audio", "wav"])
        .arg(input)
        .args(entry)
        .arg("--out")
        .arg(output)
        .output()?)
}

fn read_receipt(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn field<'rows>(
    rows: &'rows [Value],
    name: &str,
) -> Result<&'rows Value, Box<dyn std::error::Error>> {
    rows.iter()
        .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing WAV field {name}").into())
}

fn evidence(rows: &[Value], namespace: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    rows.iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        })
        .map(|used| {
            used.pointer("/location/indices")
                .cloned()
                .ok_or_else(|| format!("missing {namespace} indices").into())
        })
        .collect()
}
