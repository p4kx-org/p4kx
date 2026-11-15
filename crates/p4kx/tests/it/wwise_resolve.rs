// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx wwise resolve` end to end, over a synthetic Archive with an ATL definition and a
//! soundbank hierarchy of events, actions, containers and sounds.

use std::error::Error;
use std::path::{Path, PathBuf};

#[path = "wwise_resolve_origin_tests.rs"]
mod origin_tests;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;

struct Run {
    stdout: String,
    stderr: String,
}

fn container(kind: TestContainerKind, id: u32, children: &[u32]) -> TestHircObject {
    TestHircObject::container(kind, id, children, TestNodeBase::Full)
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let definitions = TestAtl::new("SC_Ship")
        .trigger("Play_Engine", &["Play_Engine_Start", "Play_Engine_Loop"])
        .trigger("Play_Broken", &["Play_Broken", "Play_In_No_Bank"]);
    let ship = TestBank::new(wwise_short_id("Ship"))
        .wem(5000, vec![0xA5; 40])
        .wem(5003, vec![0x3C; 16])
        .object(TestHircObject::event(
            wwise_short_id("Play_Engine_Start"),
            &[1],
        ))
        .object(TestHircObject::action(1, PLAY, 100))
        .object(container(TestContainerKind::Random, 100, &[200, 201]))
        .object(TestHircObject::sound(200, 5000, 0, 40))
        .object(TestHircObject::sound(201, 5002, 0, 24))
        .object(TestHircObject::event(
            wwise_short_id("Play_Engine_Loop"),
            &[2],
        ))
        .object(TestHircObject::action(2, PLAY, 300))
        .object(container(TestContainerKind::Switch, 300, &[301, 302]))
        .object(container(TestContainerKind::Sequence, 301, &[203, 204]))
        .object(TestHircObject::sound(203, 5003, 1, 16))
        .object(TestHircObject::sound(204, 5004, 2, 0))
        .object(container(TestContainerKind::Blend, 302, &[205]))
        .object(TestHircObject::plugin_sound(205, 6000))
        .object(TestHircObject::event(
            wwise_short_id("Play_Broken"),
            &[3, 4],
        ))
        .object(TestHircObject::action(3, PLAY, 999))
        .object(TestHircObject::action(4, PLAY, 400))
        .object(TestHircObject::raw(7, 400, [0; 8]));
    let media = TestBank::new(wwise_short_id("Media")).wem(5002, vec![0x5A; 24]);
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Ship.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::zstd(
            "Data\\Sounds\\wwise\\Ship.bnk",
            ship.bytes(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Media.bnk",
            media.bytes(),
        ))
        .write_to(&path)?;
    Ok(path)
}

fn resolve(trigger: &str, json: bool, code: i32) -> Result<Run, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .args(["wwise", "resolve"])
        .arg(&archive)
        .arg(trigger)
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
fn every_variant_is_listed_with_soundbank_wem_and_container_path_as_text() -> TestResult {
    let run = resolve("play_engine", false, 0)?;
    assert!(run.stderr.is_empty(), "no warnings: {}", run.stderr);
    insta::assert_snapshot!(run.stdout);
    Ok(())
}

#[test]
fn every_variant_is_listed_as_json_with_a_stable_shape() -> TestResult {
    let run = resolve("Play_Engine", true, 0)?;
    assert_eq!(run.stdout.lines().count(), 1, "one line of JSON");
    let mut value: serde_json::Value = serde_json::from_str(&run.stdout)?;
    remove_origins_for_payload_snapshot(&mut value)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

pub(super) fn remove_origins_for_payload_snapshot(value: &mut serde_json::Value) -> TestResult {
    let resolver = value.as_object_mut().ok_or("resolver JSON object")?;
    for field in ["fieldOrigins", "provenance"] {
        assert!(
            resolver
                .remove(field)
                .is_some_and(|value| value.is_object()),
            "structured companion {field} exists"
        );
    }
    Ok(())
}

#[test]
fn broken_links_are_listed_with_the_object_where_the_chain_breaks() -> TestResult {
    let text = resolve("Play_Broken", false, 0)?;
    insta::assert_snapshot!(text.stdout);
    let json = resolve("Play_Broken", true, 0)?;
    let value: serde_json::Value = serde_json::from_str(&json.stdout)?;
    insta::assert_snapshot!(serde_json::to_string_pretty(&value["breaks"])?);
    Ok(())
}

#[test]
fn an_undefined_trigger_is_invalid_input() -> TestResult {
    let run = resolve("Play_Undefined", true, 2)?;
    assert!(run.stdout.is_empty(), "no JSON: {}", run.stdout);
    assert_eq!(
        run.stderr.trim(),
        "error: no ATL definition has the audio trigger Play_Undefined",
        "message"
    );
    Ok(())
}
