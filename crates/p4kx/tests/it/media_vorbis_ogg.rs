// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestEntry, TestOpus, TestOpusFraming, TestVorbis};
use serde_json::Value;

use super::{TestResult, audio_export, jsonl_rows, wem_conversion};

#[test]
fn media_cli_ogg_matches_standalone_vorbis() -> TestResult {
    let original = TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .wem()
        .bytes();
    assert_ogg_parity("audio/vorbis.wem", "vorbis", original)
}

#[test]
fn media_cli_ogg_matches_standalone_opus() -> TestResult {
    let original = TestOpus::new(TestOpusFraming::Wwise, 2, 9_600)
        .declared_sample_count(960)
        .wem()?
        .smpl_loop(0, 1, 2)
        .bytes();
    assert_ogg_parity("audio/opus.wem", "opus", original)
}

fn assert_ogg_parity(entry: &str, codec: &str, original: Vec<u8>) -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(entry, original).encrypted())
        .write_to(&source)?;

    let output = folder.path().join("media");
    audio_export(&source, &output, entry)?
        .args(["--audio-format", "ogg"])
        .assert()
        .code(0);
    let index: p4kx_media::Index =
        serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    assert!(index.complete, "the Ogg request publishes a complete index");
    let directory = output.join(index.artifact_base);
    let rows = jsonl_rows(&directory.join("audio.jsonl"))?;
    let mut conversions = rows.iter().filter(|row| wem_conversion(row));
    let mapping = conversions.next().ok_or("explicit Ogg conversion")?;
    assert!(
        conversions.next().is_none(),
        "one physical WEM was converted"
    );
    let artifact = checked_artifact_path(mapping, codec)?;
    let bytes = fs::read(directory.join(artifact))?;
    assert!(
        bytes.starts_with(b"OggS"),
        "the artifact contains an Ogg stream"
    );

    let standalone = folder.path().join("standalone.ogg");
    p4kx_testkit::p4kx()?
        .args(["audio", "ogg"])
        .arg(&source)
        .arg(entry)
        .arg("--out")
        .arg(&standalone)
        .assert()
        .code(0);
    assert_eq!(
        bytes,
        fs::read(standalone)?,
        "streamed and standalone Ogg bytes match"
    );
    Ok(())
}

fn checked_artifact_path<'mapping>(
    mapping: &'mapping Value,
    codec: &str,
) -> TestResult<&'mapping str> {
    for (path, expected) in [
        ("/data/payload/requested_format", "ogg"),
        ("/data/payload/source_codec", codec),
        ("/data/payload/outcome/type", "written"),
        ("/data/payload/outcome/payload/actual_format", "ogg"),
        ("/data/payload/outcome/payload/actual_codec", codec),
    ] {
        assert_eq!(
            mapping.pointer(path).and_then(Value::as_str),
            Some(expected),
            "{path}"
        );
    }
    mapping
        .pointer("/data/payload/outcome/payload/artifact/path")
        .and_then(Value::as_str)
        .ok_or_else(|| "published Ogg artifact path".into())
}
