// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx wwise extract` and `p4kx entity audio --extract` over a synthetic Archive: file names,
//! formats, unconvertible sounds and Conflict policies.

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use lewton::inside_ogg::OggStreamReader;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestCodec, TestEntity, TestEntry,
    TestHircObject, TestVorbis, TestWem, audio_schema, entity_schema, test_signal, wwise_short_id,
};
use serde_json::Value;

#[path = "wwise_txtp_approximation.rs"]
mod txtp_approximation;
#[path = "wwise_txtp_clip_omissions.rs"]
mod txtp_clip_omissions;
#[path = "wwise_txtp_transition_omissions.rs"]
mod txtp_transition_omissions;

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;
const EMBEDDED: u8 = 0;
const STREAMED: u8 = 2;

fn vorbis() -> Vec<u8> {
    TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .wem()
        .bytes()
}

fn pcm() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(480, 1)).bytes()
}

fn xma() -> Vec<u8> {
    TestWem::encoded(TestCodec::Tag(0x0166), 2, 48_000, 1_000).bytes()
}

fn bank() -> Vec<u8> {
    let mut bank = TestBank::new(wwise_short_id("Test"))
        .wem(100, vorbis())
        .wem(101, pcm())
        .wem(102, xma())
        .wem(200, pcm())
        .object(TestHircObject::event(
            wwise_short_id("Play_Multi"),
            &[1, 2, 3, 4, 5],
        ))
        .object(TestHircObject::event(wwise_short_id("Play_Single"), &[6]));
    for (action, target) in [(1, 10), (2, 11), (3, 12), (4, 13), (5, 14)] {
        bank = bank.object(TestHircObject::action(action, PLAY, target));
    }
    bank.object(TestHircObject::sound(10, 100, EMBEDDED, 0))
        .object(TestHircObject::sound(11, 101, EMBEDDED, 0))
        .object(TestHircObject::sound(12, 102, EMBEDDED, 0))
        .object(TestHircObject::sound(13, 103, STREAMED, 0))
        .object(TestHircObject::sound(14, 104, STREAMED, 0))
        .object(TestHircObject::action(6, PLAY, 20))
        .object(TestHircObject::sound(20, 200, EMBEDDED, 0))
        .object(TestHircObject::event(wwise_short_id("Play_Twice"), &[7, 8]))
        .object(TestHircObject::action(7, PLAY, 21))
        .object(TestHircObject::action(8, PLAY, 22))
        .object(TestHircObject::sound(21, 200, EMBEDDED, 0))
        .object(TestHircObject::sound(22, 200, EMBEDDED, 0))
        .bytes()
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let ship = TestEntity::new(
        0x0079_c5d5_1678_4f8c_85ba_18ca_8f64_2af6,
        "TEST_Ship",
        "libs/foundry/records/entities/spaceships/test_ship.xml",
    )
    .audio_trigger("Play_Single")
    .audio_trigger("play_single_copy");
    let twin = TestEntity::new(
        0x0079_c5d5_1678_4f8c_85ba_18ca_8f64_2af7,
        "TEST_Twin",
        "libs/foundry/records/entities/spaceships/test_twin.xml",
    )
    .audio_trigger("Play:Case")
    .audio_trigger("Play_CASE");
    let database = audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(ship.record())
        .record(twin.record());
    let definitions = TestAtl::new("SC_Audio")
        .trigger("Play_Multi", &["Play_Multi"])
        .trigger("Play_Twice", &["Play_Twice"])
        .trigger("Play:Case", &["Play_Single"])
        .trigger("Play_CASE", &["Play_Single"])
        .trigger("Play_Single", &["Play_Single"])
        .trigger("PLAY_SINGLE_COPY", &["Play_Single"])
        .trigger("..\\Evil:x", &["Play_Single"]);
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored("Data\\Sounds\\wwise\\Test.bnk", bank()))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Media\\103.wem",
            pcm(),
        ))
        .write_to(&path)?;
    Ok(path)
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: PathBuf,
    out: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn extract(
    fixture: &Fixture,
    trigger: &str,
    flags: &[&str],
    code: i32,
) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["wwise", "extract"])
        .arg(&fixture.archive)
        .arg(trigger)
        .arg("--out")
        .arg(&fixture.out)
        .args(flags)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn names(folder: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut names: Vec<String> = fs::read_dir(folder)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, Box<dyn Error>>>()?;
    names.sort();
    Ok(names)
}

fn count(report: &Value, key: &str) -> Option<u64> {
    report.get(key)?.as_u64()
}

#[test]
fn a_trigger_with_several_sounds_is_named_by_trigger_and_wem() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "play_multi", &[], 1)?;
    assert_eq!(
        names(&fixture.out)?,
        [
            "Play_Multi_100.ogg",
            "Play_Multi_101.wav",
            "Play_Multi_102.wem",
            "Play_Multi_103.wav"
        ]
    );
    assert_eq!(report["format"], "ogg");
    assert_eq!(count(&report, "written"), Some(4));
    assert_eq!(count(&report, "unconverted"), Some(1));
    assert_eq!(count(&report, "failed"), Some(1));
    let failure = report["failures"][0]["wem_id"].as_u64();
    assert_eq!(failure, Some(104));
    let ogg = fs::read(fixture.out.join("Play_Multi_100.ogg"))?;
    OggStreamReader::new(Cursor::new(ogg))?;
    assert_eq!(fs::read(fixture.out.join("Play_Multi_102.wem"))?, xma());
    let wav = fs::read(fixture.out.join("Play_Multi_101.wav"))?;
    assert_eq!(wav.get(..4), Some(&b"RIFF"[..]));
    Ok(())
}

#[test]
fn a_trigger_with_one_sound_is_named_by_the_trigger() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Single", &["--format", "wav"], 0)?;
    assert_eq!(names(&fixture.out)?, ["Play_Single.wav"]);
    assert_eq!(count(&report, "failed"), Some(0));
    Ok(())
}

#[test]
fn a_wem_reached_through_several_variants_is_written_once() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Twice", &["--format", "wav"], 0)?;
    assert_eq!(names(&fixture.out)?, ["Play_Twice.wav"]);
    assert_eq!(count(&report, "written"), Some(1));
    assert_eq!(report["files"].as_array().map(Vec::len), Some(1));
    Ok(())
}

#[test]
fn raw_keeps_the_wem_bytes() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Single", &["--format", "raw"], 0)?;
    assert_eq!(report["format"], "raw");
    assert_eq!(report["files"][0]["format"], "wem");
    assert_eq!(fs::read(fixture.out.join("Play_Single.wem"))?, pcm());
    Ok(())
}

#[test]
fn unsafe_trigger_names_stay_inside_the_target() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture, "..\\Evil:x", &[], 0)?;
    assert_eq!(names(&fixture.out)?, ["..%5CEvil%3Ax.wav"]);
    assert!(
        !fixture
            .out
            .parent()
            .is_some_and(|parent| parent.join("Evil_x.wav").exists())
    );
    Ok(())
}

#[test]
fn conflict_policies_decide_about_existing_files() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture, "Play_Single", &[], 0)?;
    let target = fixture.out.join("Play_Single.wav");
    fs::write(&target, b"old")?;
    let skipped = extract(&fixture, "Play_Single", &[], 0)?;
    assert_eq!(count(&skipped, "kept"), Some(1));
    assert_eq!(fs::read(&target)?, b"old");
    let replaced = extract(&fixture, "Play_Single", &["--conflict", "if-changed"], 0)?;
    assert_eq!(count(&replaced, "written"), Some(1));
    let unchanged = extract(&fixture, "Play_Single", &["--conflict", "if-changed"], 0)?;
    assert_eq!(count(&unchanged, "kept"), Some(1));
    fs::write(&target, b"old")?;
    let overwritten = extract(&fixture, "Play_Single", &["--conflict", "overwrite"], 0)?;
    assert_eq!(count(&overwritten, "written"), Some(1));
    assert_ne!(fs::read(&target)?, b"old");
    Ok(())
}

#[test]
fn an_unknown_trigger_is_invalid_input() -> TestResult {
    let fixture = fixture()?;
    let output = p4kx_testkit::p4kx()?
        .args(["wwise", "extract"])
        .arg(&fixture.archive)
        .arg("Play_Nothing")
        .arg("--out")
        .arg(&fixture.out)
        .output()?;
    assert_eq!(output.status.code(), Some(2), "exit code: {output:?}");
    assert!(!fixture.out.exists());
    Ok(())
}

#[test]
fn entity_audio_extracts_every_trigger_with_distinct_names() -> TestResult {
    let fixture = fixture()?;
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "audio"])
        .arg(&fixture.archive)
        .arg("TEST_Ship")
        .arg("--extract")
        .arg(&fixture.out)
        .args(["--format", "raw"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(count(&report["extraction"], "written"), Some(2));
    assert_eq!(
        names(&fixture.out)?,
        ["PLAY_SINGLE_COPY.wem", "Play_Single.wem"]
    );
    Ok(())
}

#[test]
fn entity_audio_separates_triggers_whose_escaped_names_differ() -> TestResult {
    let fixture = fixture()?;
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "audio"])
        .arg(&fixture.archive)
        .arg("TEST_Twin")
        .arg("--extract")
        .arg(&fixture.out)
        .args(["--format", "raw"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(count(&report["extraction"], "written"), Some(2));
    assert_eq!(names(&fixture.out)?, ["Play%3ACase.wem", "Play_CASE.wem"]);
    Ok(())
}

#[test]
fn txtp_writes_one_file_per_event_next_to_the_sounds() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Single", &["--format", "wav", "--txtp"], 0)?;
    assert_eq!(
        names(&fixture.out)?,
        ["Play_Single.txtp", "Play_Single.wav"]
    );
    assert_eq!(
        fs::read_to_string(fixture.out.join("Play_Single.txtp"))?,
        "# P4KX: event Play_Single of trigger Play_Single\nPlay_Single.wav\n"
    );
    assert_eq!(count(&report["txtp"], "written"), Some(1));
    assert_eq!(count(&report["txtp"], "skipped"), Some(0));
    Ok(())
}

#[test]
fn txtp_layers_the_actions_of_an_event() -> TestResult {
    let fixture = fixture()?;
    extract(&fixture, "Play_Twice", &["--format", "wav", "--txtp"], 0)?;
    let text = fs::read_to_string(fixture.out.join("Play_Twice.txtp"))?;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.get(1..3),
        Some(&["  Play_Twice.wav", "  Play_Twice.wav"][..])
    );
    assert!(
        lines
            .get(4)
            .is_some_and(|line| line == &"group = -L2 #@layer-v"),
        "{text}"
    );
    Ok(())
}

#[test]
fn txtp_skips_an_event_with_a_sound_that_has_no_file() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Multi", &["--txtp"], 1)?;
    assert!(!fixture.out.join("Play_Multi.txtp").exists());
    assert_eq!(count(&report["txtp"], "written"), Some(0));
    assert_eq!(count(&report["txtp"], "skipped"), Some(1));
    assert_eq!(report["txtp"]["skips"][0]["event"], "Play_Multi");
    Ok(())
}

#[test]
fn without_txtp_no_txtp_is_written_or_reported() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, "Play_Single", &["--format", "wav"], 0)?;
    assert_eq!(names(&fixture.out)?, ["Play_Single.wav"]);
    assert!(report.get("txtp").is_none());
    Ok(())
}
