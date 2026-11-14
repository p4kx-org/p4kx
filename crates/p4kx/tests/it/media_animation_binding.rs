// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, TestAnimation, TestChunkFile, TestEntry, TestSkeleton, TestTrack,
};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn report(archive: &Path, output: &Path, options: &[&str]) -> TestResult<Value> {
    let result = p4kx_testkit::p4kx()?
        .args(["--json", "media", "export"])
        .arg(archive)
        .args(["--kind", "animation", "--out"])
        .arg(output)
        .args(options)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

#[test]
fn cli_binding_respects_the_shared_generation_conflict_policy() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let output = folder.path().join("out");
    let bone = TestSkeleton::controller_id(0);
    let clip = TestAnimation::new("clip.caf", 30)
        .track(TestTrack::new(bone).positions(&[(0, [0.0; 3])]))
        .caf();
    let rig = TestChunkFile::ivo()
        .chunk(0xC201_1111, 1, TestSkeleton::two_bones().compiled_bones_2())
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/clip.caf", clip))
        .entry(TestEntry::stored("Data/rig.chr", rig))
        .write_to(&archive)?;
    let original = report(&archive, &output, &[])?;
    let selected = ["--animation-skeleton", "Data/rig.chr"];
    let skipped = report(&archive, &output, &selected)?;
    assert_eq!(skipped["status"], "skipped", "keep the existing generation");
    assert_eq!(
        skipped["index"], original["index"],
        "skip preserves the exact raw export"
    );
    let updated = report(
        &archive,
        &output,
        &[selected[0], selected[1], "--conflict", "overwrite"],
    )?;
    assert_eq!(updated["status"], "published", "explicit replacement");
    let generation = updated
        .pointer("/index/artifact_base")
        .and_then(Value::as_str)
        .ok_or("generation artifact directory")?;
    let rows = fs::read_to_string(output.join(generation).join("animation.jsonl"))?;
    let mut bindings = Vec::new();
    for line in rows.lines() {
        let row: Value = serde_json::from_str(line)?;
        if row.pointer("/data/record_type") == Some(&json!("controller-binding")) {
            bindings.push(row);
        }
    }
    assert_eq!(bindings.len(), 1, "one key-bearing authored controller");
    assert_eq!(
        bindings
            .first()
            .and_then(|row| row.pointer("/data/payload/bound_bone_ordinal")),
        Some(&json!(0)),
        "the original first bone is bound",
    );
    Ok(())
}

#[test]
fn cli_entity_selection_uses_root_dataforge_and_the_shared_animation_stream() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let output = folder.path().join("out");
    let entity = p4kx_testkit::TestEntity::new(1, "CliFixture", "entities/fixture.xml")
        .geometry("Objects/Test/fixture.chr", None);
    let data = p4kx_testkit::entity_schema(p4kx_testkit::DataForgeBuilder::new())
        .record(entity.record())
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", data))
        .entry(TestEntry::stored(
            "Data/Objects/Test/fixture.chrparams",
            r#"<Params><AnimationList><Animation name="named" path="Data/Animations/walk.caf"/></AnimationList></Params>"#,
        ))
        .entry(TestEntry::stored(
            "Data/Animations/walk.caf",
            TestAnimation::new("walk.caf", 30).caf(),
        ))
        .write_to(&archive)?;
    let selected = report(&archive, &output, &["--animation-entity", "CliFixture"])?;
    assert_eq!(
        selected["status"], "published",
        "CLI publishes entity scope"
    );
    let generation = selected
        .pointer("/index/artifact_base")
        .and_then(Value::as_str)
        .ok_or("generation artifact directory")?;
    let rows = fs::read_to_string(output.join(generation).join("animation.jsonl"))?;
    let matching: Vec<_> = rows
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|row| row.pointer("/data/record_type") == Some(&json!("entity-clip")))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one authored CAF alias points to one physical clip"
    );
    assert_eq!(
        matching
            .first()
            .and_then(|row| row.pointer("/data/payload/frames_per_second")),
        Some(&json!(30)),
        "CLI selection includes original source timing",
    );
    Ok(())
}
