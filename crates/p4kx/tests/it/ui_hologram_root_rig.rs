// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_testkit::{
    PROP_ON_BONE_PORT, PROP_ON_SKIN_PORT, PROP_RIG_HUMAN, TestBone, TestEntry, TestModel,
    TestSkeleton, prop_rig_entries, test_key,
};
use serde_json::{Value, json};

use super::super::hologram_for;
use super::{TestResult, receipt, skip, source_uses, write_archive};

const ROOT: &str = "Data\\Objects\\Characters\\Human\\Test\\skeleton.chr";

fn rig(height: f32) -> Vec<u8> {
    let bone = |name: &str, parent, z| TestBone {
        name: name.to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0, 0.0, z],
    };
    TestModel::cube()
        .helpers_only()
        .with_skeleton(&TestSkeleton::new(vec![
            bone("pelvis", None, height),
            bone("root", Some(0), 1.0),
            bone("tip", Some(1), 1.0),
        ]))
        .cgf()
}

fn fixture(root: Vec<u8>) -> Vec<TestEntry> {
    let mut entries = prop_rig_entries();
    entries.retain(|entry| entry.path() != ROOT);
    entries.push(TestEntry::stored(ROOT, root).encrypted());
    entries
}

#[test]
fn the_character_root_pose_supplies_only_consumed_equipment_anchors() -> TestResult {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship.png");
    let key = test_key().to_hex();
    let wrist = format!("Body_ItemPort/{PROP_ON_SKIN_PORT}");
    let hand = format!("Body_ItemPort/{PROP_ON_BONE_PORT}");
    for attached in [true, false] {
        let mut baseline = None;
        for height in [1.0, 6.0] {
            let bytes = rig(height);
            let original = ChunkFile::parse(bytes.as_slice())?;
            let input = tempfile::tempdir_in(dir.path())?;
            let archive = write_archive(input.path(), fixture(bytes.clone()))?;
            let mut options = vec!["--size", "96", "--key", &key];
            if !attached {
                options.extend(["--exclude", &wrist, "--exclude", &hand]);
            }
            let run = hologram_for(&archive, &out, PROP_RIG_HUMAN, &options, true)?;
            if skip(
                "the_character_root_pose_supplies_only_consumed_equipment_anchors",
                &run,
            ) {
                return Ok(());
            }
            assert_eq!(run.code, Some(0), "{}", run.stderr);
            let report: Value = serde_json::from_str(&run.stdout)?;
            let rows = receipt(&out, &report)?;
            assert_inputs(&original, &rows, &report, attached)?;
            let image = fs::read(&out)?;
            if let Some(previous) = baseline.as_ref() {
                assert_eq!(
                    *previous != image,
                    attached,
                    "the pelvis affects only consumed equipment placement"
                );
            } else {
                baseline = Some(image);
            }
        }
    }
    Ok(())
}

fn assert_inputs(
    file: &ChunkFile<&[u8]>,
    rows: &[Value],
    report: &Value,
    attached: bool,
) -> TestResult {
    assert!(
        !source_uses(rows, report, "body.skinm").is_empty(),
        "skinned meshes are still admitted in both runs",
    );
    let uses = source_uses(rows, report, "skeleton.chr");
    assert_eq!(
        report.pointer("/artifactOrigin/containsEncryptedSource"),
        Some(&json!(attached.then_some(true))),
        "only an admitted rig placement carries the encrypted source",
    );
    if !attached {
        assert_eq!(
            uses,
            [] as [&Value; 0],
            "the rig is loaded but no admitted piece consumes its bind pose"
        );
        return Ok(());
    }
    assert_eq!(
        uses.len(),
        3,
        "the selected root bone's name, parent and relative pose only"
    );
    let (index, chunk) = file
        .chunks_of(ivo::COMPILED_BONES_2)
        .next()
        .ok_or("original skeleton")?;
    let records = chunk.offset() + 4;
    let name = records + 3 * 68;
    assert_site(&uses, index, "name", name..name + 7, "selector");
    assert_site(
        &uses,
        index,
        "parent",
        records + 8..records + 12,
        "selector",
    );
    assert_site(
        &uses,
        index,
        "relativePose",
        records + 12..records + 40,
        "conversion",
    );
    Ok(())
}

fn assert_site(uses: &[&Value], chunk: usize, field: &str, range: Range<u64>, role: &str) {
    let location = json!({
        "type": "subresource", "namespace": "modelBonePlacementBytes", "field": field,
        "indices": [chunk.to_string(), "0".to_owned(), range.start.to_string(), range.end.to_string()],
    });
    assert!(
        uses.iter().any(|row| row.get("location") == Some(&location)
            && row.get("role").and_then(Value::as_str) == Some(role)),
        "missing root rig source {location} as {role}: {uses:?}",
    );
}
