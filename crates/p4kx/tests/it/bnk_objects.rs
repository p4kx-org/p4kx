// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx bnk objects` end to end, on synthetic soundbanks.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_extraction::Soundbank;

use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    TestPlaylistItem,
};
use serde_json::Value;

#[path = "bnk_blend_source_fields.rs"]
mod blend_source_fields;
#[path = "bnk_dialogue_source_fields.rs"]
mod dialogue_source_fields;
#[path = "bnk_music_track_source_fields.rs"]
mod music_track_source_fields;
#[path = "bnk_random_sequence_source_fields.rs"]
mod random_sequence_source_fields;
#[path = "bnk_switch_source_fields.rs"]
mod switch_source_fields;

type TestResult = Result<(), Box<dyn Error>>;

const BANK_ENTRY: &str = "Data\\Sounds\\wwise\\SSAM_Music.bnk";

fn plain_node_prefix() -> Result<Vec<u8>, Box<dyn Error>> {
    const PLAIN_NODE_LENGTH: usize = 31;
    let original = TestBank::new(9)
        .version(154)
        .object(TestHircObject::actor_mixer(2, &[], TestNodeBase::Plain))
        .bytes();
    let original = Soundbank::parse(&original)?;
    let node = original
        .objects()
        .first()
        .ok_or("original synthetic shared node")?;
    let start = size_of::<u32>();
    let bytes = original
        .object_body(node)
        .get(start..start + PLAIN_NODE_LENGTH)
        .ok_or("checked synthetic shared node prefix")?;
    Ok(bytes.to_vec())
}

fn music_bank() -> TestBank {
    TestBank::new(0xD155_6D49)
        .object(TestHircObject::music_segment(10, &[11], &["Entry", "Exit"]))
        .object(TestHircObject::music_track(11, &[100, 101]))
        .object(TestHircObject::music_playlist_items(
            12,
            &[10],
            &[
                TestPlaylistItem::new(0, 3),
                TestPlaylistItem::new(10, 0).play_mode(0).loops(0),
                TestPlaylistItem::new(10, 0).play_mode(2).loops(3),
                TestPlaylistItem::new(10, 0).play_mode(3),
            ],
        ))
        .object(TestHircObject::music_switch(13, &[12], 0x77, &[12]))
        .object(TestHircObject::music_switch_track(
            18,
            &[102],
            0x88,
            &[5, 6],
        ))
        .object(TestHircObject::music_state_track(20, &[103], 0x99, &[7]))
        .object(TestHircObject::container(
            TestContainerKind::Switch,
            14,
            &[15],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(15, 100, 2, 4096))
        .object(TestHircObject::event(19, &[20]))
        .object(TestHircObject::raw(0xFE, 16, [0; 8]))
        .object(TestHircObject::raw(2, 17, [0; 3]))
}

fn write_bank(dir: &Path, bank: &TestBank) -> Result<PathBuf, Box<dyn Error>> {
    let file = dir.join("SSAM_Music.bnk");
    std::fs::write(&file, bank.bytes())?;
    Ok(file)
}

fn text(args: &[&str], file: &Path) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["bnk", "objects"])
        .arg(file)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let prefix = file.display().to_string();
    Ok(String::from_utf8(output.stdout)?.replace(&prefix, "[FILE]"))
}

fn json(args: &[&str], file: &Path) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["bnk", "objects"])
        .arg(file)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn ids(report: &Value) -> Vec<u64> {
    report["objects"]
        .as_array()
        .map(|objects| objects.iter().filter_map(|o| o["id"].as_u64()).collect())
        .unwrap_or_default()
}

#[test]
fn the_text_shows_every_object_in_bank_order_with_its_music_details() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    insta::assert_snapshot!(text(&[], &file)?);
    Ok(())
}

#[test]
fn the_json_has_a_stable_shape() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let mut report = json(&[], &file)?;
    report["soundbank"] = "[FILE]".into();

    insta::assert_snapshot!(serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[test]
fn a_playlist_lists_its_items_with_depth_and_loop_counts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let report = json(&["--id", "12"], &file)?;

    let playlist = &report["objects"][0];
    assert_eq!(playlist["type"], "music-playlist");
    assert_eq!(playlist["children"], serde_json::json!([10]));
    let items = playlist["detail"]["items"].as_array().ok_or("items")?;
    let depths: Vec<_> = items.iter().map(|item| item["depth"].clone()).collect();
    assert_eq!(depths, [0, 1, 1, 1]);
    assert_eq!(items[1]["segment_id"], 10);
    assert_eq!(items[0]["loops"], -1);
    assert_eq!(items[1]["loops"], 0, "endless");
    assert_eq!(items[2]["loops"], 3);
    assert_eq!(
        (items[1]["loop_min"].clone(), items[1]["loop_max"].clone()),
        (2.into(), 5.into())
    );
    let modes: Vec<_> = items.iter().map(|item| item["play_mode"].clone()).collect();
    assert_eq!(modes, [1, 0, 2, 3]);
    Ok(())
}

#[test]
fn the_text_names_play_modes_loops_and_group_kinds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let playlist = text(&["--id", "12"], &file)?;
    for expected in [
        "group of 3, sequence continuous, loops -1 (range 2..5)",
        "segment 10, random, loops endless (range 2..5)",
        "segment 10, random step, loops 3 (range 2..5)",
        "segment 10, sequence step, loops -1 (range 2..5)",
    ] {
        assert!(playlist.contains(expected), "{expected}: {playlist}");
    }
    assert!(text(&["--id", "18"], &file)?.contains("follows switch 136"));
    assert!(text(&["--id", "20"], &file)?.contains("follows state 153"));
    Ok(())
}

#[test]
fn a_segment_lists_its_duration_and_markers() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let report = json(&["--type", "music-segment"], &file)?;

    let detail = &report["objects"][0]["detail"];
    assert_eq!(detail["duration_ms"], 4000.5);
    let markers = detail["markers"].as_array().ok_or("markers")?;
    let names: Vec<_> = markers
        .iter()
        .map(|marker| marker["name"].clone())
        .collect();
    assert_eq!(names, ["Entry", "Exit"]);
    let positions: Vec<_> = markers
        .iter()
        .map(|marker| marker["position_ms"].clone())
        .collect();
    assert_eq!(positions, [1.5, 251.5]);
    Ok(())
}

#[test]
fn a_switch_container_and_a_switch_track_list_their_children_and_switches() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let container = json(&["--id", "14"], &file)?;
    assert_eq!(
        container["objects"][0]["detail"]["container_kind"],
        "switch"
    );
    assert_eq!(container["objects"][0]["children"], serde_json::json!([15]));
    assert_eq!(container["objects"][0]["parent"], 0x2C21_1B2E_u32);

    let track = json(&["--id", "0x12"], &file)?;
    let switch = &track["objects"][0]["detail"]["switch"];
    assert_eq!(switch["group_id"], 0x88);
    assert_eq!(switch["switches"], serde_json::json!([5, 6]));
    assert_eq!(
        track["objects"][0]["children"],
        serde_json::json!([102]),
        "the sources of a track"
    );
    Ok(())
}

#[test]
fn undecoded_and_malformed_objects_are_listed_and_counted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let report = json(&[], &file)?;

    assert_eq!(report["total"], 11);
    assert_eq!(
        report["layouts"],
        serde_json::json!({"decoded": 9, "undecoded": 1, "unsupported": 0, "malformed": 1})
    );
    let status = |id: u32| {
        report["objects"]
            .as_array()
            .and_then(|objects| objects.iter().find(|o| o["id"] == id))
            .map(|object| (object["status"].clone(), object["error"].is_string()))
    };
    assert_eq!(status(16), Some(("undecoded".into(), false)));
    assert_eq!(status(17), Some(("malformed".into(), true)));
    Ok(())
}

#[test]
fn objects_of_a_newer_bank_version_are_unsupported() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bank = TestBank::new(1)
        .version(160)
        .object(TestHircObject::event(1, &[2]))
        .object(TestHircObject::raw(7, 3, [0; 8]));
    let file = write_bank(dir.path(), &bank)?;

    let report = json(&[], &file)?;

    assert_eq!(
        report["layouts"],
        serde_json::json!({"decoded": 0, "undecoded": 0, "unsupported": 2, "malformed": 0})
    );
    assert_eq!(report["objects"][0]["status"], "unsupported");
    assert!(text(&[], &file)?.contains("2 unsupported"));
    Ok(())
}

#[test]
fn a_supported_actor_mixer_exposes_its_original_parent_and_ordered_children() -> TestResult {
    let dir = tempfile::tempdir()?;
    let children = [37, 19, 37];
    let bank = TestBank::new(12).object(TestHircObject::actor_mixer(
        1,
        &children,
        TestNodeBase::Full,
    ));
    let file = write_bank(dir.path(), &bank)?;
    let report = json(&["--type", "actor-mixer"], &file)?;
    let actor = &report["objects"][0];
    assert_eq!(actor["type"], "actor-mixer");
    assert_eq!(actor["status"], "decoded");
    assert_eq!(actor["parent"], 0x2C21_1B2E_u32);
    assert_eq!(actor["children"], serde_json::json!(children));
    assert!(actor["detail"]["children_range"]["start"].is_string());
    Ok(())
}

#[test]
fn the_type_and_id_filters_combine_and_a_hex_id_is_accepted() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    assert_eq!(ids(&json(&["--type", "sound"], &file)?), [15, 17]);
    assert_eq!(ids(&json(&["--id", "0x0B"], &file)?), [11]);
    let empty = json(&["--type", "sound", "--id", "11"], &file)?;
    assert_eq!(empty["matched"], 0);
    assert_eq!(empty["objects"], serde_json::json!([]));
    assert_eq!(empty["next_offset"], Value::Null);
    assert_eq!(empty["total"], 11, "the bank is still counted");
    assert_eq!(
        empty["layouts"]["malformed"], 1,
        "also the unmatched objects"
    );
    Ok(())
}

#[test]
fn pages_follow_bank_order_and_name_the_next_offset() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path(), &music_bank())?;

    let first = json(&["--limit", "4"], &file)?;
    assert_eq!(ids(&first), [10, 11, 12, 13]);
    assert_eq!(first["next_offset"], 4);
    assert_eq!(first["matched"], 11);

    let last = json(&["--limit", "4", "--offset", "8"], &file)?;
    assert_eq!(ids(&last), [19, 16, 17]);
    assert_eq!(last["next_offset"], Value::Null);

    let summary = text(&["--limit", "4", "--offset", "4"], &file)?;
    assert!(
        summary.contains("showing 5-8 of 11; next page: --offset 8"),
        "{summary}"
    );
    Ok(())
}

#[test]
fn a_soundbank_entry_of_an_archive_is_read_like_a_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(BANK_ENTRY, music_bank().bytes()))
        .write_to(&archive)?;

    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["bnk", "objects"])
        .arg(&archive)
        .arg("data/sounds/wwise/ssam_music.BNK")
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["soundbank"], BANK_ENTRY);
    assert_eq!(report["total"], 11);
    Ok(())
}

#[test]
fn a_file_that_is_no_soundbank_fails() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("x.bnk");
    std::fs::write(&file, b"not a bank")?;

    let output = p4kx_testkit::p4kx()?
        .args(["bnk", "objects"])
        .arg(&file)
        .output()?;

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    Ok(())
}
