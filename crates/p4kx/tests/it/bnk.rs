// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx bnk list|extract` end to end, for soundbank Entries of an Archive and files on disk.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestCodec, TestEntry, TestHircObject, TestWem, test_signal,
};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

const BANK_ENTRY: &str = "Data\\Sounds\\wwise\\SSAM_Test.bnk";
const FRAMES: u32 = 4_800;

fn pcm_wem() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(FRAMES, 1)).bytes()
}

fn vorbis_wem() -> Vec<u8> {
    TestWem::encoded(TestCodec::Vorbis, 2, 48_000, 72_000).bytes()
}

fn bank() -> Vec<u8> {
    TestBank::new(0xD155_6D49)
        .wem(300, vorbis_wem())
        .wem(100, pcm_wem())
        .wem(200, b"PLUG\0\0\0\0".to_vec())
        .wem(400, vorbis_wem().get(..120).unwrap_or_default().to_vec())
        .bytes()
}

fn write_archive(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(BANK_ENTRY, bank()))
        .write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

#[test]
fn list_of_an_entry_prints_id_codec_and_duration_per_wem() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let output = p4kx()?
        .args(["bnk", "list"])
        .arg(&archive)
        .arg("data/sounds/wwise/ssam_test.BNK")
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    insta::assert_snapshot!(String::from_utf8(output.stdout)?);
    Ok(())
}

#[test]
fn extract_writes_every_wem_named_by_its_id() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("bank");

    p4kx()?
        .args(["bnk", "extract"])
        .arg(&archive)
        .arg(BANK_ENTRY)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("extracted 4 of 4 WEMs"))
        .stdout(predicate::str::contains(
            "1 of them only the prefetched start of a streamed WEM",
        ));

    assert_eq!(fs::read(out.join("100.wem"))?, pcm_wem(), "WEM 100");
    assert_eq!(fs::read(out.join("300.wem"))?, vorbis_wem(), "WEM 300");
    assert_eq!(fs::read(out.join("200.wem"))?, b"PLUG\0\0\0\0", "WEM 200");
    assert_eq!(
        fs::read(out.join("400.wem"))?.len(),
        120,
        "the prefetched start as the bank holds it"
    );
    for id in [100, 200, 300, 400] {
        assert!(
            out.join(format!("{id}.wem.p4kx-origin.jsonl")).is_file(),
            "missing raw companion for {id}",
        );
    }
    assert_eq!(fs::read_dir(&out)?.count(), 8, "media and their companions");
    Ok(())
}

#[test]
fn extract_as_wav_converts_and_names_the_codecs_it_cannot() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("SSAM_Test.bnk");
    fs::write(&file, bank())?;
    let out = dir.path().join("wav");

    p4kx()?
        .args(["bnk", "extract"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .args(["--format", "wav"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("extracted 1 of 4 WEMs"))
        .stderr(predicate::str::contains(
            "WEM 300: the Wwise Vorbis data is invalid",
        ))
        .stderr(predicate::str::contains("WEM 200: not a sound"))
        .stderr(predicate::str::contains(
            "WEM 400 is only the prefetched start of a streamed WEM",
        ));

    let wav = fs::read(out.join("100.wav"))?;
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
    assert!(
        !out.join("300.wav").exists(),
        "no WAV for the invalid Vorbis WEM"
    );
    assert_eq!(fs::read_dir(&out)?.count(), 1, "no partial files");
    Ok(())
}

#[test]
fn extract_prints_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("SSAM_Test.bnk");
    fs::write(&file, bank())?;
    let out = dir.path().join("wav");

    let output = p4kx()?
        .args(["--json", "bnk", "extract"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .args(["--format", "wav"])
        .output()?;

    assert_eq!(output.status.code(), Some(1), "partial failure");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["format"], "wav", "format");
    assert_eq!(json["extracted"][0]["id"], 100, "WEM 100 written");
    assert_eq!(
        json["extracted"][0]["file"],
        out.join("100.wav").display().to_string(),
        "its file"
    );
    let failed: Vec<&serde_json::Value> = json["failed"]
        .as_array()
        .ok_or("failed array")?
        .iter()
        .map(|failure| &failure["id"])
        .collect();
    assert_eq!(failed, [200, 300, 400], "failed IDs");
    Ok(())
}

#[test]
fn a_corrupt_bank_names_the_section_and_fails() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("broken.bnk");
    let bytes = bank();
    fs::write(&file, bytes.get(..bytes.len() - 3).unwrap_or_default())?;

    p4kx()?
        .args(["bnk", "list"])
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "the soundbank section DATA is truncated",
        ));
    p4kx()?
        .args(["bnk", "list"])
        .arg(dir.path().join("missing.bnk"))
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn a_missing_entry_is_invalid_input_and_an_unreadable_archive_an_archive_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    p4kx()?
        .args(["bnk", "extract"])
        .arg(&archive)
        .arg("Data\\missing.bnk")
        .arg("--out")
        .arg(dir.path().join("out"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no Entry named Data\\missing.bnk"));
    let broken = dir.path().join("broken.p4k");
    fs::write(&broken, b"not an archive")?;
    p4kx()?
        .args(["bnk", "list"])
        .arg(&broken)
        .arg(BANK_ENTRY)
        .assert()
        .code(3);
    Ok(())
}

#[test]
fn a_malformed_hirc_object_does_not_stop_listing_or_extracting() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("odd.bnk");
    let bytes = TestBank::new(1)
        .wem(100, pcm_wem())
        .object(TestHircObject::raw(2, 7, [1, 0]))
        .bytes();
    fs::write(&file, bytes)?;

    p4kx()?
        .args(["bnk", "list"])
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicate::str::is_match(
            r"(?m)^ +100  WEM +PCM +0\.100 s$",
        )?);
    p4kx()?
        .args(["bnk", "extract"])
        .arg(&file)
        .arg("--out")
        .arg(dir.path().join("out"))
        .assert()
        .code(0);
    assert_eq!(
        fs::read(dir.path().join("out").join("100.wem"))?,
        pcm_wem(),
        "WEM 100"
    );
    Ok(())
}
