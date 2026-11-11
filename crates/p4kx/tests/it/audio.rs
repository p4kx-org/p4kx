// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx audio info|wav` end to end, for Entries of an Archive and for files on disk.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem, test_signal};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

const PCM_ENTRY: &str = "Data\\Sounds\\wwise\\Media\\100.wem";
const VORBIS_ENTRY: &str = "Data\\Sounds\\wwise\\Media\\200.wem";
const AAC_ENTRY: &str = "Data\\Sounds\\wwise\\Media\\300.wem";
const FRAMES: u32 = 24_000;

fn pcm_wem() -> TestWem {
    TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &test_signal(FRAMES, 2))
}

fn write_archive(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(PCM_ENTRY, pcm_wem().bytes()))
        .entry(TestEntry::stored(
            VORBIS_ENTRY,
            TestWem::encoded(TestCodec::Vorbis, 1, 48_000, 72_000).bytes(),
        ))
        .entry(TestEntry::stored(
            AAC_ENTRY,
            TestWem::encoded(TestCodec::Tag(0xAAC0), 1, 48_000, 960).bytes(),
        ))
        .write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

/// Returns the interleaved samples of a canonical 44-byte-header PCM WAV.
fn wav_samples(wav: &[u8]) -> Vec<i16> {
    wav.get(44..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(i16::from_le_bytes)
        .collect()
}

#[test]
fn info_of_an_entry_prints_codec_channels_rate_and_duration() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .args(["audio", "info"])
        .arg(&archive)
        .arg("data/sounds/wwise/media/200.WEM")
        .assert()
        .code(0)
        .stdout(predicate::str::is_match(
            r"(?m)^Sound: +Data\\Sounds\\wwise\\Media\\200\.wem$",
        )?)
        .stdout(predicate::str::is_match(r"(?m)^Codec: +Wwise Vorbis$")?)
        .stdout(predicate::str::is_match(r"(?m)^Channels: +1$")?)
        .stdout(predicate::str::is_match(r"(?m)^Sample rate: 48000 Hz$")?)
        .stdout(predicate::str::is_match(r"(?m)^Duration: +1\.500 s$")?);
    Ok(())
}

#[test]
fn info_of_a_file_prints_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("100.wem");
    fs::write(&file, pcm_wem().bytes())?;

    let output = p4kx()?
        .args(["--json", "audio", "info"])
        .arg(&file)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success");
    let mut json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    super::json_audio::remove_info_metadata(&mut json)?;
    assert_eq!(
        json,
        serde_json::json!({
            "sound": file.display().to_string(),
            "codec": "pcm",
            "codec_name": "PCM",
            "format_tag": 0xFFFE,
            "channels": 2,
            "sample_rate": 48_000,
            "sample_count": FRAMES,
            "duration_seconds": 0.5,
            "loop": null,
        }),
        "the JSON shape"
    );
    Ok(())
}

#[test]
fn wav_of_a_pcm_entry_holds_the_known_signal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("100.wav");

    p4kx()?
        .args(["audio", "wav"])
        .arg(&archive)
        .arg(PCM_ENTRY)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("bytes"));

    let wav = fs::read(&out)?;
    assert_eq!(wav.get(..4), Some(&b"RIFF"[..]), "a RIFF file");
    assert_eq!(wav.get(22..24), Some(&2u16.to_le_bytes()[..]), "stereo");
    assert_eq!(
        wav.get(24..28),
        Some(&48_000u32.to_le_bytes()[..]),
        "48 kHz"
    );
    assert_eq!(
        wav_samples(&wav),
        test_signal(FRAMES, 2),
        "the samples are the known signal"
    );
    Ok(())
}

#[test]
fn wav_of_a_big_endian_file_prints_json_and_holds_the_known_signal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("console.wem");
    fs::write(
        &file,
        TestWem::pcm(TestCodec::Pcm, 1, 48_000, &test_signal(1_000, 1))
            .big_endian()
            .bytes(),
    )?;
    let out = dir.path().join("console.wav");

    let output = p4kx()?
        .args(["--json", "audio", "wav"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["sample_count"], 1_000, "frames");
    assert_eq!(json["size"], 44 + 2_000, "size");
    assert_eq!(
        wav_samples(&fs::read(&out)?),
        test_signal(1_000, 1),
        "the samples are the known signal"
    );
    Ok(())
}

#[test]
fn wav_of_another_codec_names_it_and_fails() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("300.wav");

    p4kx()?
        .args(["audio", "wav"])
        .arg(&archive)
        .arg(AAC_ENTRY)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "converting AAC to WAV is not supported",
        ));

    assert!(!out.exists(), "no WAV is written");
    Ok(())
}

#[test]
fn a_corrupt_wem_fails_with_a_typed_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("broken.wem");
    fs::write(&file, pcm_wem().bytes().get(..40).unwrap_or_default())?;

    p4kx()?
        .args(["audio", "info"])
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("the WEM is truncated"));
    Ok(())
}

#[test]
fn a_missing_entry_or_file_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .args(["audio", "info"])
        .arg(&archive)
        .arg("Data\\missing.wem")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry named Data\\missing.wem"));
    p4kx()?
        .args(["audio", "wav"])
        .arg(dir.path().join("missing.wem"))
        .arg("--out")
        .arg(dir.path().join("missing.wav"))
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn an_unreadable_archive_is_an_archive_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    fs::write(&archive, b"not an archive")?;

    p4kx()?
        .args(["audio", "info"])
        .arg(&archive)
        .arg(PCM_ENTRY)
        .assert()
        .code(3);
    Ok(())
}

#[test]
fn unreadable_entry_data_or_an_unreadable_file_is_a_read_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::deflated(PCM_ENTRY, pcm_wem().bytes()).with_payload(vec![0xFF; 64]))
        .write_to(&archive)?;
    let folder = dir.path().join("folder.wem");
    fs::create_dir(&folder)?;

    p4kx()?
        .args(["audio", "info"])
        .arg(&archive)
        .arg(PCM_ENTRY)
        .assert()
        .code(3);
    p4kx()?
        .args(["audio", "info"])
        .arg(&folder)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("cannot read"));
    Ok(())
}
