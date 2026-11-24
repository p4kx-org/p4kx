// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx wwise events` end to end, over a synthetic Archive with Entities, ATL definitions and
//! soundbanks.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject,
    TestRecord, TestStruct, TestType, audio_schema, entity_schema, wwise_short_id,
};

#[path = "wwise_event_origins.rs"]
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

fn ship(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!(
            "libs/foundry/records/entities/spaceships/{}.xml",
            class.to_lowercase()
        ),
    )
}

fn database() -> Vec<u8> {
    audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(
            ship(0x1, "TEST_Avenger")
                .audio_trigger("Play_Engine")
                .audio_trigger("play_engine")
                .audio_trigger("Play_Shield")
                .record(),
        )
        .record(
            ship(0x2, "TEST_Gladius")
                .audio_trigger("PLAY_ENGINE")
                .sound_effect("Shield Power Up", "Play_Shield")
                .record(),
        )
        .record(
            ship(0x3, "TEST_Cutlass")
                .audio_trigger("Play_Engine")
                .record(),
        )
        .record(ship(0x4, "TEST_Mute").record())
        .build()
}

fn broken_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .record(TestRecord::new(
            0x9000_0000_0000_4000_8000_0000_0000_0009,
            "EntityClassDefinition",
            "EntityClassDefinition.TEST_Broken",
            "libs/foundry/records/entities/spaceships/test_broken.xml",
        ))
        .build()
}

fn write_archive(dir: &Path, database: Option<Vec<u8>>) -> Result<PathBuf, Box<dyn Error>> {
    let ship_atl = TestAtl::new("SC_Ship")
        .trigger("Play_Engine", &["Play_Engine_Loop", "Play_Engine_Tail"])
        .trigger("Play_Shield", &["Play_Shield_Up"])
        .trigger("Play_Nowhere", &["Play_In_No_Bank"])
        .trigger("Play_Silent", &[])
        .trigger("Stop_Engine", &["Stop_Engine_All"]);
    let voice = TestAtl::new("SC_Voice").localised_trigger("Play_Voice", "Play_DX_Hello");
    let hostile =
        TestAtl::new("SC_Hostile").trigger("Play_Evil\n\u{1b}[31mred", &["Play_Evil_Event"]);
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Ship.xml",
            ship_atl.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Libs\\GameAudio\\SC_Voice.xml",
            voice.to_xml(),
        ))
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Hostile.xml",
            hostile.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Ship.bnk",
            bank(&["Play_Engine_Loop", "Play_Engine_Tail", "Play_Shield_Up"]),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Shared.bnk",
            bank(&["Play_Shield_Up", "Play_Evil_Event"]),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\English(US)\\Voice.bnk",
            bank(&["Play_DX_Hello"]),
        ));
    if let Some(database) = database {
        builder = builder.entry(TestEntry::zstd("Data\\Game2.dcb", database));
    }
    let path = dir.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(path)
}

fn events(
    args: &[&str],
    json: bool,
    with_database: bool,
    code: i32,
) -> Result<Run, Box<dyn Error>> {
    events_over(args, json, with_database.then(database), code)
}

fn events_over(
    args: &[&str],
    json: bool,
    database: Option<Vec<u8>>,
    code: i32,
) -> Result<Run, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), database)?;
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["wwise", "events"])
        .arg(&archive)
        .args(args)
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

fn text(args: &[&str]) -> Result<Run, Box<dyn Error>> {
    events(args, false, true, 0)
}

#[test]
fn a_substring_matches_trigger_names_ignoring_case() -> TestResult {
    let run = text(&["--search", "ENGINE"])?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn an_event_name_finds_the_trigger_that_posts_it() -> TestResult {
    let run = text(&["--search", "engine_tail"])?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn a_glob_matches_the_whole_name() -> TestResult {
    let run = text(&["--search", "play_s*"])?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn without_a_search_every_trigger_is_listed_with_its_reason_when_unmatched() -> TestResult {
    let run = text(&[])?;
    assert!(run.stdout.contains("Play_Nowhere"), "{}", run.stdout);
    assert!(
        run.stdout
            .contains("unmatched: no soundbank holds the Wwise event"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("unmatched: the ATL definition posts no Wwise event"),
        "{}",
        run.stdout
    );
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn results_come_in_pages_and_the_summary_names_the_next_offset() -> TestResult {
    let first = text(&["--limit", "2"])?;
    let second = text(&["--limit", "2", "--offset", "2"])?;
    let last = text(&["--limit", "2", "--offset", "6"])?;
    insta::assert_snapshot!("page_1", first.stdout);
    insta::assert_snapshot!("page_2", second.stdout);
    insta::assert_snapshot!("page_3", last.stdout);
    Ok(())
}

#[test]
fn the_number_of_listed_entities_is_bounded_but_the_count_is_not() -> TestResult {
    let run = text(&["--search", "Play_Engine", "--entities", "1"])?;
    assert!(
        run.stdout
            .contains("Entities (3): TEST_Avenger, and 2 more"),
        "{}",
        run.stdout
    );
    let none = text(&["--search", "Play_Engine", "--entities", "0"])?;
    assert!(none.stdout.contains("Entities (3)\n"), "{}", none.stdout);
    Ok(())
}

#[test]
fn json_has_a_stable_shape() -> TestResult {
    let run = events(&["--search", "shield", "--entities", "1"], true, true, 0)?;
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let mut value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    let fields = value.as_object_mut().ok_or("Wwise Events JSON object")?;
    for key in ["fieldOrigins", "provenance"] {
        assert!(fields.remove(key).is_some(), "Wwise Events includes {key}");
    }
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn json_pages_carry_the_total_and_the_next_offset() -> TestResult {
    let run = events(&["--limit", "2"], true, true, 0)?;
    let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(value["total"], 7);
    assert_eq!(value["next_offset"], 2);
    assert_eq!(value["triggers"].as_array().map(Vec::len), Some(2));
    let last = events(&["--limit", "2", "--offset", "6"], true, true, 0)?;
    let value: serde_json::Value = serde_json::from_str(&last.stdout)?;
    assert!(value["next_offset"].is_null());
    Ok(())
}

#[test]
fn no_match_prints_only_the_summary() -> TestResult {
    let run = text(&["--search", "nothing_like_this"])?;
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn no_entities_skips_the_dataforge() -> TestResult {
    let run = events(&["--search", "shield", "--no-entities"], false, false, 0)?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    assert!(!run.stdout.contains("Entities"), "{}", run.stdout);
    let json = events(&["--search", "shield", "--no-entities"], true, false, 0)?;
    let value: serde_json::Value = serde_json::from_str(&json.stdout)?;
    assert!(value["triggers"][0]["entities"].is_null());
    Ok(())
}

#[test]
fn an_archive_without_a_dataforge_still_lists_the_triggers_with_a_warning_and_exits_1() -> TestResult
{
    let run = events(&["--search", "shield"], false, false, 1)?;
    assert!(
        run.stderr.starts_with("warning: no Entities listed"),
        "{}",
        run.stderr
    );
    assert!(run.stdout.contains("Play_Shield"), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_failed_entity_lookup_is_told_apart_from_no_entities_in_the_json() -> TestResult {
    let failed = events(&["--search", "shield"], true, false, 1)?;
    let value: serde_json::Value = serde_json::from_str(&failed.stdout)?;
    assert!(value["entities_skipped"].is_string(), "{value}");
    assert!(value["triggers"][0]["entities"].is_null());
    let off = events(&["--search", "shield", "--no-entities"], true, false, 0)?;
    let value: serde_json::Value = serde_json::from_str(&off.stdout)?;
    assert!(value["entities_skipped"].is_null(), "{value}");
    Ok(())
}

#[test]
fn an_empty_page_does_not_read_the_dataforge() -> TestResult {
    let run = events(&["--search", "nothing_like_this"], true, false, 0)?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert!(value["entities_skipped"].is_null(), "{value}");
    assert_eq!(value["fieldOrigins"], serde_json::json!({}));
    assert_eq!(value["provenance"]["sources"], serde_json::json!([]));
    assert_eq!(value["provenance"]["uses"], serde_json::json!([]));
    Ok(())
}

#[test]
fn unreadable_entity_records_are_warned_about_and_counted() -> TestResult {
    let run = events_over(&["--search", "shield"], true, Some(broken_database()), 0)?;
    assert!(
        run.stderr
            .contains("1 Entity Records cannot be read and name no trigger"),
        "{}",
        run.stderr
    );
    let value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(value["unreadable_records"], 1);
    let clean = events(&["--search", "shield"], true, true, 0)?;
    let value: serde_json::Value = serde_json::from_str(&clean.stdout)?;
    assert_eq!(value["unreadable_records"], 0);
    Ok(())
}

#[test]
fn hostile_names_are_printed_on_one_line_without_control_characters() -> TestResult {
    let run = text(&["--search", "evil"])?;
    assert!(
        run.stdout.contains("Play_Evil\\n\u{fffd}[31mred"),
        "{:?}",
        run.stdout
    );
    assert!(!run.stdout.contains('\u{1b}'), "{:?}", run.stdout);
    Ok(())
}

#[test]
fn a_limit_of_zero_is_refused() -> TestResult {
    let run = events(&["--limit", "0"], false, true, 2)?;
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    Ok(())
}
