// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx entity audio` end to end, over a synthetic Archive with an Entity, ATL definitions and
//! soundbanks and, with `--ignored`, over the real Archive `P4KX_DATA_P4K` names.

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject,
    audio_schema, entity_schema, real_archive_path, wwise_short_id,
};

#[path = "entity_audio_origin_tests.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;

struct Run {
    stdout: String,
    stderr: String,
}

fn bank(events: &[&str]) -> Vec<u8> {
    events
        .iter()
        .zip(1_u32..)
        .fold(TestBank::new(0xB0), |bank, (event, action)| {
            bank.object(TestHircObject::action(action, PLAY, 0x100 + action))
                .object(TestHircObject::event(wwise_short_id(event), &[action]))
        })
        .bytes()
}

fn write_archive(dir: &Path, broken: bool) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let ship = TestEntity::new(
        0x0079_c5d5_1678_4f8c_85ba_18ca_8f64_2af6,
        "TEST_Ship",
        "libs/foundry/records/entities/spaceships/test_ship.xml",
    )
    .audio_trigger("Play_Engine")
    .audio_trigger("Play_Undefined")
    .sound_effect("Shield Power Up", "Play_Shield")
    .audio_trigger("Play_Nowhere")
    .audio_trigger("Play_Voice")
    .audio_trigger("Play_Engine");
    let database = audio_schema(entity_schema(DataForgeBuilder::new())).record(ship.record());
    let definitions = TestAtl::new("SC_Ship")
        .trigger("Play_Engine", &["Play_Engine_Loop", "Play_Engine_Tail"])
        .trigger("Play_Shield", &["Play_Shield_Up"])
        .trigger("Play_Nowhere", &["Play_In_No_Bank"])
        .bank("Ship.bnk");
    let voice = TestAtl::new("SC_Voice").localised_trigger("Play_Voice", "Play_DX_Hello");
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Ship.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Libs\\GameAudio\\SC_Voice.xml",
            voice.to_xml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Ship.bnk",
            bank(&["Play_Engine_Loop", "Play_Engine_Tail", "Play_Shield_Up"]),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Shared.bnk",
            bank(&["Play_Shield_Up"]),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Voice.bnk",
            bank(&["Play_DX_Hello"]),
        ));
    if broken {
        builder = builder.entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Broken.bnk",
            b"RIFF".to_vec(),
        ));
    }
    let path = dir.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(path)
}

fn audio(archive: &Path, entity: &str, json: bool, code: i32) -> Result<Run, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["entity", "audio"])
        .arg(archive)
        .arg(entity)
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

fn sample(entity: &str, json: bool, code: i32) -> Result<Run, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), false)?;
    audio(&archive, entity, json, code)
}

#[test]
fn the_triggers_are_listed_with_events_and_soundbanks_as_text() -> TestResult {
    let run = sample("TEST_Ship", false, 0)?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn the_triggers_are_listed_as_json_with_a_stable_shape() -> TestResult {
    let run = sample("0079c5d5-1678-4f8c-85ba-18ca8f642af6", true, 0)?;
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let mut value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    super::wwise_resolve::remove_origins_for_payload_snapshot(&mut value)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn unreadable_soundbanks_are_warned_about_and_listed_in_the_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), true)?;
    let text = audio(&archive, "TEST_Ship", false, 0)?;
    assert_eq!(
        text.stderr.trim(),
        "warning: Data\\Sounds\\wwise\\Broken.bnk: the soundbank cannot be parsed: \
         not a soundbank: the BKHD section is missing",
        "one warning"
    );
    assert!(
        text.stdout
            .ends_with("5 triggers: 3 matched, 2 unmatched\n"),
        "the rest still resolves: {}",
        text.stdout
    );
    let json = audio(&archive, "TEST_Ship", true, 0)?;
    assert!(
        json.stderr.is_empty(),
        "no warnings with --json: {}",
        json.stderr
    );
    let value: serde_json::Value = serde_json::from_str(&json.stdout)?;
    assert_eq!(
        value["issues"],
        serde_json::json!([{
            "path": "Data\\Sounds\\wwise\\Broken.bnk",
            "kind": "soundbank",
            "message": "the soundbank cannot be parsed: not a soundbank: the BKHD section is missing",
        }]),
        "issues"
    );
    Ok(())
}

#[test]
fn an_unknown_entity_is_invalid_input() -> TestResult {
    let run = sample("No_Such_Ship", true, 2)?;
    assert!(run.stdout.is_empty(), "no JSON: {}", run.stdout);
    assert_eq!(
        run.stderr.trim(),
        "error: no Entity has the record path, GUID or class name No_Such_Ship",
        "message"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn real_ships_resolve_most_of_their_triggers() -> TestResult {
    let archive = real_archive_path()?;
    for entity in [
        "ORIG_890Jump",
        "AEGS_Gladius",
        "ANVL_Hornet_F7C",
        "RSI_Constellation_Andromeda",
        "TMBL_Cyclone",
    ] {
        let run = audio(&archive, entity, true, 0)?;
        let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
        let triggers = value["counts"]["triggers"].as_u64().ok_or("triggers")?;
        let matched = value["counts"]["matched"].as_u64().ok_or("matched")?;
        assert!(triggers >= 5, "{entity}: {triggers} triggers");
        assert!(
            matched * 10 >= triggers * 9,
            "{entity}: {matched} of {triggers} triggers matched"
        );
    }
    Ok(())
}
