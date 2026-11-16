// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx wwise resolve` and `p4kx entity audio --sounds` end to end for streamed sounds located as
//! loose WEM Entries and for localised sounds in a chosen language, over a synthetic Archive.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject,
    audio_schema, entity_schema, wwise_short_id,
};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;

struct Run {
    stdout: String,
    stderr: String,
}

fn voice_bank(event: &str, wem: u32) -> Vec<u8> {
    TestBank::new(wwise_short_id(event))
        .object(TestHircObject::event(wwise_short_id(event), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::sound(10, wem, 2, 0))
        .bytes()
}

fn common_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id("Common"))
        .wem(7001, vec![0x11; 8])
        .object(TestHircObject::event(
            wwise_short_id("Play_Long"),
            &[1, 2, 3],
        ))
        .object(TestHircObject::action(1, PLAY, 200))
        .object(TestHircObject::action(2, PLAY, 201))
        .object(TestHircObject::action(3, PLAY, 202))
        .object(TestHircObject::sound(200, 7000, 2, 0))
        .object(TestHircObject::sound(201, 7001, 1, 8))
        .object(TestHircObject::sound(202, 7002, 2, 0))
        .object(TestHircObject::event(wwise_short_id("Play_Line"), &[4]))
        .object(TestHircObject::action(4, PLAY, 300))
        .object(TestHircObject::language_sound(300, 9000, 2, 0))
        .bytes()
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let ship = TestEntity::new(
        0x0079_c5d5_1678_4f8c_85ba_18ca_8f64_2af6,
        "TEST_Ship",
        "libs/foundry/records/entities/spaceships/test_ship.xml",
    )
    .audio_trigger("Play_Long")
    .audio_trigger("Play_Voice")
    .audio_trigger("Play_Only");
    let database = audio_schema(entity_schema(DataForgeBuilder::new())).record(ship.record());
    let definitions = TestAtl::new("SC_Audio")
        .trigger("Play_Long", &["Play_Long"])
        .trigger("Play_Line", &["Play_Line"])
        .localised_trigger("Play_Voice", "Play_DX_Hello")
        .localised_trigger("Play_Only", "Play_DX_Only");
    let loose = |name: &str, len: usize| TestEntry::stored(name, vec![0x7E; len]);
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Common.bnk",
            common_bank(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Voice.bnk",
            voice_bank("Play_DX_Hello", 8000),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\German\\Voice.bnk",
            voice_bank("Play_DX_Hello", 8000),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Only.bnk",
            voice_bank("Play_DX_Only", 8500),
        ))
        .entry(loose("Data\\Sounds\\wwise\\Media\\7000.wem", 33))
        .entry(loose("Data\\Sounds\\wwise\\7001.wem", 12))
        .entry(loose(
            "Data\\Sounds\\wwise\\Media\\English(US)\\8000.wem",
            21,
        ))
        .entry(loose("Data\\Sounds\\wwise\\Media\\German\\8000.wem", 22))
        .entry(loose(
            "Data\\Sounds\\wwise\\Media\\English(US)\\8500.wem",
            23,
        ))
        .entry(loose(
            "Data\\Sounds\\wwise\\Media\\English(US)\\9000.wem",
            41,
        ))
        .entry(loose("Data\\Sounds\\wwise\\Media\\German\\9000.wem", 42))
        .write_to(&path)?;
    Ok(path)
}

fn run(args: &[&str], json: bool, code: i32) -> Result<Run, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let (head, tail) = args.split_at(2);
    let (target, flags) = tail.split_first().ok_or("a target")?;
    let output = command
        .args(head)
        .arg(&archive)
        .arg(target)
        .args(flags)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    let archive_text = archive.display().to_string();
    let json_text = serde_json::to_string(&archive_text)?;
    let json_text = json_text.trim_matches('"');
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?
            .replace(json_text, "<archive>")
            .replace(&archive_text, "<archive>"),
        stderr: String::from_utf8(output.stderr)?,
    })
}

#[test]
fn streamed_sources_show_their_loose_wem_entry_as_text() -> TestResult {
    let run = run(&["wwise", "resolve", "Play_Long"], false, 0)?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn a_localised_trigger_is_resolved_in_the_chosen_language_as_text() -> TestResult {
    let run = run(
        &["wwise", "resolve", "Play_Voice", "--lang", "german"],
        false,
        0,
    )?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn the_default_language_is_the_fallback_when_none_is_chosen() -> TestResult {
    let run = run(&["wwise", "resolve", "Play_Line"], false, 0)?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn the_fallback_language_is_marked_where_the_chosen_one_has_no_localisation() -> TestResult {
    let run = run(
        &["wwise", "resolve", "Play_Only", "--lang", "German"],
        false,
        0,
    )?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn a_missing_localisation_is_listed_as_a_broken_link() -> TestResult {
    let run = run(
        &[
            "wwise",
            "resolve",
            "Play_Only",
            "--lang",
            "German",
            "--fallback-lang",
            "German",
        ],
        false,
        0,
    )?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn the_localised_variants_are_listed_as_json_with_a_stable_shape() -> TestResult {
    let run = run(
        &["wwise", "resolve", "Play_Line", "--lang", "German"],
        true,
        0,
    )?;
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let mut value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    super::wwise_resolve::remove_origins_for_payload_snapshot(&mut value)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn a_language_the_archive_has_no_folder_for_is_invalid_input() -> TestResult {
    let run = run(
        &["wwise", "resolve", "Play_Long", "--lang", "Klingon"],
        false,
        2,
    )?;
    assert!(
        run.stderr.contains("English(US), German"),
        "the language folders are listed: {}",
        run.stderr
    );
    assert_eq!(run.stdout, "");
    Ok(())
}

#[test]
fn entity_audio_lists_the_sounds_with_source_and_location() -> TestResult {
    let run = run(
        &[
            "entity",
            "audio",
            "TEST_Ship",
            "--sounds",
            "--lang",
            "German",
        ],
        false,
        0,
    )?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn entity_audio_lists_the_sounds_as_json_with_a_stable_shape() -> TestResult {
    let run = run(
        &[
            "entity",
            "audio",
            "TEST_Ship",
            "--sounds",
            "--lang",
            "German",
        ],
        true,
        0,
    )?;
    let mut value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    super::wwise_resolve::remove_origins_for_payload_snapshot(&mut value)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn entity_audio_without_sounds_lists_no_sounds() -> TestResult {
    let run = run(&["entity", "audio", "TEST_Ship"], true, 0)?;
    let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert!(value.get("language").is_none(), "{value}");
    assert!(!run.stdout.contains("\"sounds\""), "{}", run.stdout);
    Ok(())
}
