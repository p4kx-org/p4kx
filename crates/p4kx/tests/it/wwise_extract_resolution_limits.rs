// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::process::Output;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestContainerKind, TestEntity, TestEntry,
    TestHircObject, TestNodeBase, audio_schema, entity_schema, wwise_short_id,
};
use serde_json::Value;

use super::{EMBEDDED, Fixture, PLAY, TestResult, names, pcm};

const CUT: &str = "Play_DepthCut";
const COMPLETE: &str = "Play_Audible";
const ENTITY: &str = "TEST_Audio_Depth";
const DIRECT_ACTION: u32 = 1;
const COMPLETE_ACTION: u32 = 2;
const ROOT_CONTAINER: u32 = 10;
const AUDIBLE_SOUND: u32 = 11;
const CHAIN_START: u32 = 12;
const CHAIN_LENGTH: u32 = 64;
const AUDIBLE_WEM: u32 = 100;
const CUT_WEM: u32 = 101;
const VISIT_LIMIT: &str = "4";
const BUDGET_ARGS: [&str; 2] = ["--max-visits", VISIT_LIMIT];
const PRIOR_BYTES: &[u8] = b"prior export payload";
const PRIOR_FILES: [&str; 3] = [
    "Play_DepthCut.txtp",
    "Play_DepthCut.txtp.p4kx-origin.jsonl",
    "Play_DepthCut.wav",
];

#[test]
fn explicit_visit_budget_refuses_wwise_exports_before_any_publication() -> TestResult {
    let fixture = fixture()?;
    check_partial_resolution(&fixture)?;
    assert_budget_failure(&extract(&fixture, &BUDGET_ARGS)?)?;
    assert!(!fixture.out.exists(), "no new output directory");

    fs::create_dir(&fixture.out)?;
    for name in PRIOR_FILES {
        fs::write(fixture.out.join(name), PRIOR_BYTES)?;
    }
    assert_budget_failure(&extract(
        &fixture,
        &["--txtp", "--max-visits", VISIT_LIMIT],
    )?)?;
    assert_eq!(names(&fixture.out)?, PRIOR_FILES, "no new export artifacts");
    for name in PRIOR_FILES {
        assert_eq!(
            fs::read(fixture.out.join(name))?,
            PRIOR_BYTES,
            "the previous output {name} is unchanged"
        );
    }
    Ok(())
}

#[test]
fn explicit_visit_budget_refuses_the_entity_audio_batch_before_any_publication() -> TestResult {
    let fixture = fixture()?;
    check_entity_inspection(&fixture)?;
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "audio"])
        .arg(&fixture.archive)
        .arg(ENTITY)
        .arg("--extract")
        .arg(&fixture.out)
        .args(["--format", "wav", "--conflict", "overwrite"])
        .args(BUDGET_ARGS)
        .output()?;
    assert_budget_failure(&output)?;
    assert!(!fixture.out.exists(), "the batch publishes neither trigger");
    Ok(())
}

#[test]
fn default_resolution_exports_a_long_path_and_both_wav_files() -> TestResult {
    let fixture = fixture()?;
    let output = extract(&fixture, &["--txtp"])?;
    assert!(
        output.status.success(),
        "complete export succeeds: {output:?}"
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report.get("written").and_then(Value::as_u64),
        Some(2),
        "both WEMs are converted"
    );
    assert_eq!(
        report.get("breaks").and_then(Value::as_u64),
        Some(0),
        "the default walk follows the whole path"
    );
    let playlist = fs::read_to_string(fixture.out.join(format!("{CUT}.txtp")))?;
    for wem in [AUDIBLE_WEM, CUT_WEM] {
        let wav = format!("{CUT}_{wem}.wav");
        assert!(
            fixture.out.join(&wav).is_file(),
            "the export contains {wav}"
        );
        assert!(playlist.contains(&wav), "the playlist references {wav}");
    }
    Ok(())
}

fn check_partial_resolution(fixture: &Fixture) -> TestResult {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["wwise", "resolve"])
        .arg(&fixture.archive)
        .arg(CUT)
        .args(BUDGET_ARGS)
        .output()?;
    assert!(
        output.status.success(),
        "inspection still reports: {output:?}"
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    check_capped_sounds(&report)
}

fn check_entity_inspection(fixture: &Fixture) -> TestResult {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "audio"])
        .arg(&fixture.archive)
        .arg(ENTITY)
        .arg("--sounds")
        .args(BUDGET_ARGS)
        .output()?;
    assert!(
        output.status.success(),
        "entity inspection still reports: {output:?}"
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let triggers = report
        .get("triggers")
        .and_then(Value::as_array)
        .ok_or("entity triggers")?;
    let sounds_for = |name: &str| {
        triggers
            .iter()
            .find(|trigger| trigger["name"] == name)
            .and_then(|trigger| trigger.get("sounds"))
            .ok_or("inspected trigger sounds")
    };
    check_capped_sounds(sounds_for(CUT)?)?;
    assert_eq!(
        sounds_for(COMPLETE)?["truncated_by_limits"],
        false,
        "each trigger reports its own traversal status"
    );
    Ok(())
}

fn check_capped_sounds(report: &Value) -> TestResult {
    assert_eq!(
        report["truncated_by_limits"], true,
        "inspection identifies the reached variants as partial"
    );
    let variants = report["variants"].as_array().ok_or("resolved variants")?;
    let [audible] = variants.as_slice() else {
        return Err(format!("expected one audible branch: {variants:?}").into());
    };
    assert_eq!(audible["wem_id"], AUDIBLE_WEM, "the first branch is usable");
    let breaks = report["breaks"].as_array().ok_or("reported breaks")?;
    let [cut] = breaks.as_slice() else {
        return Err(format!("expected one traversal cut: {breaks:?}").into());
    };
    assert_eq!(
        cut["kind"], "too_many_objects",
        "the other branch reaches the caller's visit budget"
    );
    assert_eq!(
        cut["message"],
        budget_reason(),
        "the original diagnostic is included"
    );
    Ok(())
}

fn extract(fixture: &Fixture, flags: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["wwise", "extract"])
        .arg(&fixture.archive)
        .arg(CUT)
        .arg("--out")
        .arg(&fixture.out)
        .args(["--format", "wav", "--conflict", "overwrite"])
        .args(flags)
        .output()?)
}

fn assert_budget_failure(output: &Output) -> TestResult {
    assert_eq!(output.status.code(), Some(1), "export refused: {output:?}");
    assert!(output.stdout.is_empty(), "there is no success JSON report");
    let stderr = std::str::from_utf8(&output.stderr)?;
    assert!(
        stderr.contains(CUT) && stderr.contains(&budget_reason()),
        "stderr identifies the actual cut and its trigger: {stderr}"
    );
    Ok(())
}

fn budget_reason() -> String {
    format!("the trigger exceeds the visit budget of {VISIT_LIMIT}, leaving references unfollowed")
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let out = directory.path().join("out");
    let definitions = TestAtl::new("AudioDepth")
        .trigger(COMPLETE, &[COMPLETE])
        .trigger(CUT, &[CUT]);
    let entity = TestEntity::new(
        0x0079_c5d5_1678_4f8c_85ba_18ca_8f64_2af8,
        ENTITY,
        "libs/foundry/records/entities/test_audio_depth.xml",
    )
    .audio_trigger(COMPLETE)
    .audio_trigger(CUT);
    let database = audio_schema(entity_schema(DataForgeBuilder::new())).record(entity.record());
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", database.build()))
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/AudioDepth.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored("Data/Sounds/wwise/Depth.bnk", bank()))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: directory,
        archive,
        out,
    })
}

fn bank() -> Vec<u8> {
    let terminal = CHAIN_START + CHAIN_LENGTH;
    let mut bank = TestBank::new(wwise_short_id("Depth"))
        .version(154)
        .wem(AUDIBLE_WEM, pcm())
        .wem(CUT_WEM, pcm())
        .object(TestHircObject::event(wwise_short_id(CUT), &[DIRECT_ACTION]))
        .object(TestHircObject::event(
            wwise_short_id(COMPLETE),
            &[COMPLETE_ACTION],
        ))
        .object(TestHircObject::action(DIRECT_ACTION, PLAY, ROOT_CONTAINER))
        .object(TestHircObject::action(COMPLETE_ACTION, PLAY, terminal))
        .object(sequence(ROOT_CONTAINER, &[AUDIBLE_SOUND, CHAIN_START]))
        .object(TestHircObject::sound(
            AUDIBLE_SOUND,
            AUDIBLE_WEM,
            EMBEDDED,
            0,
        ))
        .object(TestHircObject::sound(terminal, CUT_WEM, EMBEDDED, 0));
    for id in CHAIN_START..terminal {
        bank = bank.object(sequence(id, &[id + 1]));
    }
    bank.bytes()
}

fn sequence(id: u32, children: &[u32]) -> TestHircObject {
    TestHircObject::container(
        TestContainerKind::Sequence,
        id,
        children,
        TestNodeBase::Plain,
    )
}
