// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx audio ogg` end to end, for Entries of an Archive and for files on disk, and
//! `p4kx bnk extract --format ogg`.

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use lewton::inside_ogg::OggStreamReader;
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestCodebook, TestCodec, TestEntry, TestVorbis, TestWem, test_signal,
};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

const VORBIS_ENTRY: &str = "Data\\Sounds\\wwise\\Media\\300.wem";
const PCM_ENTRY: &str = "Data\\Sounds\\wwise\\Media\\301.wem";
const AOTUV_ONLY: u16 = 97;

fn short_packets(count: usize) -> TestVorbis {
    TestVorbis::new(2, 48_000, &vec![false; count])
}

fn write_archive(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            VORBIS_ENTRY,
            short_packets(376).sample_count(48_000).wem().bytes(),
        ))
        .entry(TestEntry::stored(
            PCM_ENTRY,
            TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(100, 1)).bytes(),
        ))
        .write_to(&path)?;
    Ok(path)
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

/// Returns the granule position of the last page of an Ogg file.
fn last_granule(ogg: &[u8]) -> Option<u64> {
    let start = ogg.windows(4).rposition(|window| window == b"OggS")?;
    Some(u64::from_le_bytes(
        ogg.get(start + 6..start + 14)?.try_into().ok()?,
    ))
}

/// Decodes an Ogg Vorbis file and returns its channels, sample rate and frames.
fn decode(ogg: Vec<u8>) -> Result<(u8, u32, usize), Box<dyn Error>> {
    let mut reader = OggStreamReader::new(Cursor::new(ogg))?;
    let mut frames = 0;
    while let Some(packet) = reader.read_dec_packet()? {
        frames += packet.first().map_or(0, Vec::len);
    }
    Ok((
        reader.ident_hdr.audio_channels,
        reader.ident_hdr.audio_sample_rate,
        frames,
    ))
}

#[test]
fn ogg_of_a_vorbis_entry_plays_for_the_sound_s_duration() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("300.ogg");

    p4kx()?
        .args(["audio", "ogg"])
        .arg(&archive)
        .arg("data/sounds/wwise/media/300.WEM")
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("as Ogg Vorbis"))
        .stdout(predicate::str::contains("1.000 s"))
        .stdout(predicate::str::contains("source origins:"));

    assert!(
        dir.path().join("300.ogg.p4kx-origin.jsonl").is_file(),
        "text mode publishes its companion"
    );
    let ogg = fs::read(&out)?;
    assert_eq!(ogg.get(..4), Some(&b"OggS"[..]), "an Ogg file");
    assert_eq!(last_granule(&ogg), Some(48_000), "one second at 48 kHz");
    assert_eq!(
        decode(ogg)?,
        (2, 48_000, 48_000),
        "a decoder plays one second"
    );
    Ok(())
}

#[test]
fn ogg_of_a_file_prints_json_naming_the_codebooks() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("aotuv.wem");
    let sound = TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .codebook(TestCodebook::Library(AOTUV_ONLY));
    fs::write(&file, sound.wem().bytes())?;
    let out = dir.path().join("aotuv.ogg");

    let output = p4kx()?
        .args(["--json", "audio", "ogg"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let ogg = fs::read(&out)?;
    let frames = 576 + 1024 + 576 + 128;
    assert_eq!(json["codebooks"], "aotuv", "codebooks");
    assert_eq!(json["sample_count"], frames, "frames");
    assert_eq!(json["duration_seconds"], 0.096, "duration");
    assert_eq!(json["size"], ogg.len(), "size");
    assert_eq!(decode(ogg)?, (1, 24_000, frames), "decoded");
    Ok(())
}

#[test]
fn ogg_of_another_codec_names_it_and_fails() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("301.ogg");

    p4kx()?
        .args(["audio", "ogg"])
        .arg(&archive)
        .arg(PCM_ENTRY)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "converting PCM to Ogg is not supported",
        ));

    assert!(!out.exists(), "no Ogg is written");
    Ok(())
}

#[test]
fn a_vorbis_wem_without_usable_codebooks_fails_without_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("broken.wem");
    fs::write(
        &file,
        short_packets(4)
            .codebook(TestCodebook::Library(598))
            .wem()
            .bytes(),
    )?;
    let out = dir.path().join("broken.ogg");

    p4kx()?
        .args(["audio", "ogg"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("fits no codebooks"));

    assert!(!out.exists(), "no Ogg is written");
    Ok(())
}

#[test]
fn bnk_extract_as_ogg_rebuilds_the_vorbis_wems_and_names_the_rest() -> TestResult {
    let dir = tempfile::tempdir()?;
    let vorbis = short_packets(376).sample_count(48_000).wem().bytes();
    let bank = TestBank::new(0xD155_6D49)
        .wem(7, vorbis.clone())
        .wem(
            8,
            TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(100, 1)).bytes(),
        )
        .wem(9, vorbis.get(..120).unwrap_or_default().to_vec())
        .bytes();
    let file = dir.path().join("SSAM_Test.bnk");
    fs::write(&file, bank)?;
    let out = dir.path().join("ogg");

    p4kx()?
        .args(["bnk", "extract"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .args(["--format", "ogg"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("extracted 1 of 3 WEMs"))
        .stderr(predicate::str::contains(
            "WEM 8: converting PCM to Ogg is not supported",
        ))
        .stderr(predicate::str::contains(
            "WEM 9 is only the prefetched start",
        ));

    let ogg = fs::read(out.join("7.ogg"))?;
    assert_eq!(last_granule(&ogg), Some(48_000), "one second at 48 kHz");
    assert_eq!(
        decode(ogg)?,
        (2, 48_000, 48_000),
        "a decoder plays one second"
    );
    assert_eq!(fs::read_dir(&out)?.count(), 1, "nothing else");
    Ok(())
}
