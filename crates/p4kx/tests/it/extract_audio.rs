// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx extract --convert --audio` over a synthetic Archive with WEM sounds and a soundbank.

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use lewton::inside_ogg::OggStreamReader;
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestCodec, TestEntry, TestVorbis, TestWem, test_signal,
};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

const VORBIS: &str = "Data\\Sounds\\wwise\\Media\\500.wem";
const PCM: &str = "Data\\Sounds\\wwise\\Media\\600.wem";
const XMA: &str = "Data\\Sounds\\wwise\\Media\\700.wem";
const BANK: &str = "Data\\Sounds\\wwise\\Ship.bnk";
const FRAMES: u32 = 480;

fn vorbis() -> Vec<u8> {
    TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .wem()
        .bytes()
}

fn pcm() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(FRAMES, 1)).bytes()
}

fn xma() -> Vec<u8> {
    TestWem::encoded(TestCodec::Tag(0x0166), 2, 48_000, 1_000).bytes()
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: PathBuf,
    out: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let bank = TestBank::new(1)
        .wem(10, vorbis())
        .wem(20, pcm())
        .wem(30, xma())
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(VORBIS, vorbis()))
        .entry(TestEntry::stored(PCM, pcm()))
        .entry(TestEntry::zstd(XMA, xma()))
        .entry(TestEntry::zstd(BANK, bank))
        .write_to(&archive)?;
    let out = dir.path().join("out");
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn extract(fixture: &Fixture) -> Result<Command, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .arg("extract")
        .arg(&fixture.archive)
        .args(["--filter", "Sounds"])
        .arg("--out")
        .arg(&fixture.out);
    Ok(command)
}

fn names(folder: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut names: Vec<String> = fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    Ok(names)
}

fn media(fixture: &Fixture) -> PathBuf {
    fixture.out.join("Data/Sounds/wwise/Media")
}

fn bank_folder(fixture: &Fixture) -> PathBuf {
    fixture.out.join("Data/Sounds/wwise/Ship")
}

fn ogg_samples(ogg: Vec<u8>) -> Result<usize, Box<dyn Error>> {
    let mut reader = OggStreamReader::new(Cursor::new(ogg))?;
    let mut samples = 0;
    while let Some(packet) = reader.read_dec_packet_itl()? {
        samples += packet.len();
    }
    Ok(samples)
}

#[test]
fn convert_writes_ogg_and_wav_and_unpacks_the_soundbank_by_default() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .arg("--convert")
        .assert()
        .code(1)
        .stderr(predicate::str::contains(format!("not converted {XMA}")))
        .stderr(predicate::str::contains(
            "converting XMA2 to WAV is not supported",
        ))
        .stderr(predicate::str::contains("Ship.bnk WEM 30"))
        .stdout(predicate::str::contains(
            "extracted 4 (3 converted, 2 not converted)",
        ));

    check_default_loose_media(&fixture)?;
    assert_eq!(
        names(&bank_folder(&fixture))?,
        [
            "10.ogg",
            "10.ogg.p4kx-origin.jsonl",
            "20.wav",
            "20.wav.p4kx-origin.jsonl",
            "30.wem",
            "30.wem.p4kx-origin.jsonl",
        ],
        "the soundbank's WEMs by ID"
    );
    assert!(
        fixture.out.join("Data/Sounds/wwise/Ship.bnk").is_file(),
        "the soundbank"
    );
    Ok(())
}

fn check_default_loose_media(fixture: &Fixture) -> TestResult {
    let output = media(fixture);
    assert_eq!(
        names(&output)?,
        [
            "500.ogg",
            "500.ogg.p4kx-origin.jsonl",
            "600.wav",
            "600.wav.p4kx-origin.jsonl",
            "700.wem",
            "700.wem.p4kx-origin.jsonl",
        ],
        "loose sounds"
    );
    assert!(
        ogg_samples(fs::read(output.join("500.ogg"))?)? > 0,
        "playable"
    );
    let wav = fs::read(output.join("600.wav"))?;
    let samples: Vec<i16> = wav
        .get(44..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(i16::from_le_bytes)
        .collect();
    assert_eq!(samples, test_signal(FRAMES, 1), "the known signal");
    assert_eq!(fs::read(output.join("700.wem"))?, xma(), "unchanged");
    Ok(())
}

#[test]
fn audio_wav_and_raw_choose_the_format() -> TestResult {
    let fixture = fixture()?;

    let result = extract(&fixture)?
        .args(["--convert", "--audio", "wav", "--json"])
        .assert()
        .code(1);
    let report: serde_json::Value = serde_json::from_slice(&result.get_output().stdout)?;
    check_sound_artifacts(
        &report,
        &fixture.out,
        &[
            "Data/Sounds/wwise/Media/500.wav",
            "Data/Sounds/wwise/Media/600.wav",
            "Data/Sounds/wwise/Ship/10.wav",
            "Data/Sounds/wwise/Ship/20.wav",
        ],
    )?;
    let raw = report["raw_artifacts"]
        .as_array()
        .ok_or("raw audio artifacts")?;
    let paths: Vec<_> = raw
        .iter()
        .filter_map(|item| item["path"].as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "Data/Sounds/wwise/Media/700.wem",
            "Data/Sounds/wwise/Ship.bnk",
            "Data/Sounds/wwise/Ship/30.wem",
        ],
        "the loose fallback, the original bank and the embedded fallback"
    );
    assert_eq!(
        names(&media(&fixture))?,
        [
            "500.wav",
            "500.wav.p4kx-origin.jsonl",
            "600.wav",
            "600.wav.p4kx-origin.jsonl",
            "700.wem",
            "700.wem.p4kx-origin.jsonl",
        ],
        "WAV"
    );
    assert_eq!(
        names(&bank_folder(&fixture))?,
        [
            "10.wav",
            "10.wav.p4kx-origin.jsonl",
            "20.wav",
            "20.wav.p4kx-origin.jsonl",
            "30.wem",
            "30.wem.p4kx-origin.jsonl",
        ],
        "WAV"
    );

    extract_and_check_raw_audio(&fixture)
}

fn extract_and_check_raw_audio(fixture: &Fixture) -> TestResult {
    let raw = fixture.out.with_file_name("raw");
    p4kx_testkit::p4kx()?
        .arg("extract")
        .arg(&fixture.archive)
        .args(["--filter", "Sounds", "--convert", "--audio", "raw", "--out"])
        .arg(&raw)
        .assert()
        .code(0);
    let raw_media = raw.join("Data/Sounds/wwise/Media");
    assert_eq!(
        names(&raw_media)?,
        [
            "500.wem",
            "500.wem.p4kx-origin.jsonl",
            "600.wem",
            "600.wem.p4kx-origin.jsonl",
            "700.wem",
            "700.wem.p4kx-origin.jsonl",
        ],
        "unchanged"
    );
    assert_eq!(
        names(&raw.join("Data/Sounds/wwise/Ship"))?,
        [
            "10.wem",
            "10.wem.p4kx-origin.jsonl",
            "20.wem",
            "20.wem.p4kx-origin.jsonl",
            "30.wem",
            "30.wem.p4kx-origin.jsonl",
        ],
        "unpacked unchanged"
    );
    Ok(())
}

#[test]
fn audio_needs_convert() -> TestResult {
    let fixture = fixture()?;

    extract(&fixture)?
        .args(["--audio", "wav"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--convert"));
    Ok(())
}

#[test]
fn the_json_report_lists_the_sounds_that_were_not_converted() -> TestResult {
    let fixture = fixture()?;

    let output = extract(&fixture)?
        .args(["--convert", "--json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let report: serde_json::Value = serde_json::from_slice(&output)?;
    assert_eq!(report["converted"], 3, "{report}");
    check_sound_artifacts(
        &report,
        &fixture.out,
        &[
            "Data/Sounds/wwise/Media/500.ogg",
            "Data/Sounds/wwise/Media/600.wav",
            "Data/Sounds/wwise/Ship/10.ogg",
            "Data/Sounds/wwise/Ship/20.wav",
        ],
    )?;
    let unconverted = report["unconverted"].as_array().ok_or("unconverted")?;
    let paths: Vec<_> = unconverted.iter().map(|failure| &failure["path"]).collect();
    assert_eq!(paths, [XMA, BANK], "{report}");
    let error = unconverted[0]["error"].as_str().unwrap_or_default();
    assert!(error.contains("XMA2"), "{error}");
    Ok(())
}

fn check_sound_artifacts(report: &serde_json::Value, target: &Path, paths: &[&str]) -> TestResult {
    let artifacts = report
        .get("sound_artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or("sound artifacts")?;
    assert_eq!(
        artifacts.len(),
        paths.len(),
        "one receipt per converted sound"
    );
    for (artifact, expected) in artifacts.iter().zip(paths.iter().copied()) {
        check_sound_artifact(artifact, target, expected)?;
    }
    Ok(())
}

fn check_sound_artifact(
    artifact: &serde_json::Value,
    target: &Path,
    expected_path: &str,
) -> TestResult {
    assert_eq!(
        artifact.get("path").and_then(serde_json::Value::as_str),
        Some(expected_path),
        "target-relative sound path"
    );
    let companion = artifact
        .get("origin_receipt")
        .and_then(serde_json::Value::as_str)
        .ok_or("target-relative receipt")?;
    let receipt = fs::read_to_string(target.join(companion))?;
    let binding: serde_json::Value =
        serde_json::from_str(receipt.lines().next().ok_or("binding")?)?;
    let written_digest = binding
        .pointer("/binding/sha256")
        .ok_or("bound artifact digest")?;
    let reported_digest = artifact
        .pointer("/digest/sha256")
        .ok_or("reported artifact digest")?;
    assert_eq!(
        written_digest, reported_digest,
        "JSON reports the measured binding"
    );
    Ok(())
}

#[test]
fn a_second_run_with_if_changed_keeps_every_converted_sound() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture)?.arg("--convert").assert().code(1);

    extract(&fixture)?
        .args(["--convert", "--conflict", "if-changed"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(format!(
            "skipped {VORBIS}: unchanged"
        )))
        .stdout(predicate::str::contains(format!(
            "skipped {BANK}: unchanged"
        )))
        .stdout(predicate::str::contains(
            "extracted 0 (0 converted, 2 not converted), skipped 4",
        ));
    Ok(())
}
