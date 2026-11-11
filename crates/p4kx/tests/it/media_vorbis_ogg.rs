// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::Codec;
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestOpus, TestOpusFraming, TestVorbis};
use serde_json::Value;

use super::{TestResult, audio_export, jsonl_rows, wem_conversion};

#[test]
fn media_cli_ogg_matches_standalone_vorbis() -> TestResult {
    let original = TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .wem()
        .bytes();
    assert_audio_parity("audio/vorbis.wem", Codec::Vorbis, Container::Ogg, original)
}

#[test]
fn media_cli_ogg_matches_standalone_opus() -> TestResult {
    assert_audio_parity(
        "audio/opus.wem",
        Codec::Opus,
        Container::Ogg,
        opus_fixture()?,
    )
}

#[test]
fn media_cli_wav_matches_standalone_opus() -> TestResult {
    assert_audio_parity(
        "audio/opus.wem",
        Codec::Opus,
        Container::Wav,
        opus_fixture()?,
    )
}

fn opus_fixture() -> TestResult<Vec<u8>> {
    Ok(TestOpus::new(TestOpusFraming::Wwise, 2, 9_600)
        .declared_sample_count(960)
        .wem()?
        .smpl_loop(0, 1, 2)
        .bytes())
}

#[derive(Clone, Copy)]
enum Container {
    Ogg,
    Wav,
}

impl Container {
    const fn name(self) -> &'static str {
        match self {
            Self::Ogg => "ogg",
            Self::Wav => "wav",
        }
    }

    const fn signature(self) -> &'static [u8] {
        match self {
            Self::Ogg => b"OggS",
            Self::Wav => b"RIFF",
        }
    }

    const fn output_codec(self, codec: Codec) -> Codec {
        match self {
            Self::Ogg => codec,
            Self::Wav => Codec::Pcm,
        }
    }
}

fn assert_audio_parity(
    entry: &str,
    codec: Codec,
    format: Container,
    original: Vec<u8>,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(entry, original).encrypted())
        .write_to(&source)?;

    let output = folder.path().join("media");
    audio_export(&source, &output, entry)?
        .args(["--audio-format", format.name()])
        .assert()
        .code(0);
    let index: p4kx_media::Index =
        serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    assert!(
        index.complete,
        "the audio request publishes a complete index"
    );
    let directory = output.join(index.artifact_base);
    let rows = jsonl_rows(&directory.join("audio.jsonl"))?;
    let mut conversions = rows.iter().filter(|row| wem_conversion(row));
    let mapping = conversions.next().ok_or("explicit audio conversion")?;
    assert!(
        conversions.next().is_none(),
        "one physical WEM was converted"
    );
    let artifact = checked_artifact_path(mapping, codec, format)?;
    let bytes = fs::read(directory.join(artifact))?;
    assert!(
        bytes.starts_with(format.signature()),
        "the artifact has the requested container"
    );

    let standalone = folder.path().join(format!("standalone.{}", format.name()));
    p4kx_testkit::p4kx()?
        .args(["audio", format.name()])
        .arg(&source)
        .arg(entry)
        .arg("--out")
        .arg(&standalone)
        .assert()
        .code(0);
    assert_eq!(
        bytes,
        fs::read(standalone)?,
        "streamed and standalone audio bytes match"
    );
    Ok(())
}

fn checked_artifact_path(mapping: &Value, codec: Codec, format: Container) -> TestResult<&str> {
    for (path, expected) in [
        ("/data/payload/requested_format", format.name()),
        ("/data/payload/source_codec", codec.id()),
        ("/data/payload/outcome/type", "written"),
        ("/data/payload/outcome/payload/actual_format", format.name()),
        (
            "/data/payload/outcome/payload/actual_codec",
            format.output_codec(codec).id(),
        ),
    ] {
        assert_eq!(
            mapping.pointer(path).and_then(Value::as_str),
            Some(expected),
            "{path}"
        );
    }
    if codec == Codec::Opus {
        assert!(
            mapping
                .pointer("/data/payload/outcome/payload/opus_inputs/checked_data")
                .is_some(),
            "Opus conversion keeps the checked source packet range"
        );
    }
    mapping
        .pointer("/data/payload/outcome/payload/artifact/path")
        .and_then(Value::as_str)
        .ok_or_else(|| "published audio artifact path".into())
}
