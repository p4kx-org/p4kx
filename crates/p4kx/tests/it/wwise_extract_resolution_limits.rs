// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::process::Output;

use p4kx_assembly::audio::MAX_PATH_DEPTH;
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
const AUDIBLE_WEM: u32 = 100;
const CUT_WEM: u32 = 101;
const PRIOR_BYTES: &[u8] = b"prior export payload";
const PRIOR_FILES: [&str; 3] = [
    "Play_DepthCut.txtp",
    "Play_DepthCut.txtp.p4kx-origin.jsonl",
    "Play_DepthCut.wav",
];

#[test]
fn traversal_depth_cut_refuses_wwise_exports_before_any_publication() -> TestResult {
    let fixture = fixture()?;
    check_partial_resolution(&fixture)?;
    assert_depth_failure(&extract(&fixture, &[])?)?;
    assert!(!fixture.out.exists(), "no new output directory");

    fs::create_dir(&fixture.out)?;
    for name in PRIOR_FILES {
        fs::write(fixture.out.join(name), PRIOR_BYTES)?;
    }
    assert_depth_failure(&extract(&fixture, &["--txtp"])?)?;
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
fn traversal_depth_cut_refuses_the_entity_audio_batch_before_any_publication() -> TestResult {
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
        .output()?;
    assert_depth_failure(&output)?;
    assert!(!fixture.out.exists(), "the batch publishes neither trigger");
    Ok(())
}

fn check_partial_resolution(fixture: &Fixture) -> TestResult {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["wwise", "resolve"])
        .arg(&fixture.archive)
        .arg(CUT)
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
        cut["kind"], "too_deep",
        "the other branch reaches the depth bound"
    );
    assert_eq!(
        cut["message"],
        depth_reason(),
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

fn assert_depth_failure(output: &Output) -> TestResult {
    assert_eq!(output.status.code(), Some(1), "export refused: {output:?}");
    assert!(output.stdout.is_empty(), "there is no success JSON report");
    let stderr = std::str::from_utf8(&output.stderr)?;
    assert!(
        stderr.contains(CUT) && stderr.contains(&depth_reason()),
        "stderr identifies the actual cut and its trigger: {stderr}"
    );
    Ok(())
}

fn depth_reason() -> String {
    format!("the path is deeper than {MAX_PATH_DEPTH} objects")
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
        .entry(TestEntry::stored("Data/Sounds/wwise/Depth.bnk", bank()?))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: directory,
        archive,
        out,
    })
}

fn bank() -> Result<Vec<u8>, Box<dyn Error>> {
    let terminal = CHAIN_START + u32::try_from(MAX_PATH_DEPTH)?;
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
    Ok(bank.bytes())
}

fn sequence(id: u32, children: &[u32]) -> TestHircObject {
    TestHircObject::container(
        TestContainerKind::Sequence,
        id,
        children,
        TestNodeBase::Plain,
    )
}
