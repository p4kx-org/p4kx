// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{
    ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, TestModel, TestTrack,
    check_glb, test_key,
};
use serde_json::{Value, json};

use super::{TestResult, entry_path, field_origin, field_uses};
use crate::animation_support::{IDENTITY, SKIN, SKIN_GEOMETRY, animation_names, rotating, tip};

const CAF: &str = "Data/Animations/Objects/Test/same.caf";
const DBA: &str = "Data/Animations/Objects/Test/z_clips.dba";
const FAILED: &str = "Data/Animations/Objects/Test/failed.caf";
const DBA_CLIP: &str = "fixture/same.caf";
const CAF_DATA: u32 = 0xA949_6CB5;
const CAF_DATA_VERSION: u32 = 0x900;
const STORED_FLAG: u32 = 1 << 1;
const ADDITIVE_FLAG: u32 = 1;

#[test]
fn report_binds_same_names_by_selected_clip_with_an_omitted_glb_animation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("fixture.p4k");
    let dba = fixture(&input)?;
    let output = directory.path().join("observed.glb");
    let result = command(&input, Some(SKIN), &output, true)?;
    let report = serde_json::from_slice::<Value>(&result)?;
    assert_eq!(
        report.pointer("/animations/0/name"),
        Some(&json!("same")),
        "CAF name"
    );
    assert_eq!(
        report.pointer("/animations/1/name"),
        Some(&json!("same")),
        "DBA name"
    );
    assert_eq!(
        report.pointer("/animations/0/channels"),
        Some(&json!(0)),
        "CAF has no matching channels"
    );
    assert_eq!(
        report.pointer("/animations/1/channels"),
        Some(&json!(1)),
        "DBA channel count"
    );
    assert_eq!(
        report.pointer("/animations/0/duration_seconds"),
        Some(&json!(0.5)),
        "CAF duration includes an unbound track"
    );
    assert_eq!(
        report.pointer("/animations/1/duration_seconds"),
        Some(&json!(2.0)),
        "DBA duration"
    );
    for field in ["name", "duration_seconds", "additive"] {
        assert_entry(&report, &format!("/animations/0/{field}"), CAF, false)?;
        assert_entry(&report, &format!("/animations/1/{field}"), DBA, true)?;
    }
    assert_baked_origins_unknown(&report)?;
    let name = clip_location(&report, "/animations/1/name", "databasePath")?;
    assert_source_bytes(name, 1, DBA_CLIP.as_bytes(), &dba)?;
    let duration = clip_location(&report, "/animations/1/duration_seconds", "end_frame")?;
    assert_source_bytes(duration, 1, &60_u32.to_le_bytes(), &dba)?;
    let additive = clip_location(&report, "/animations/1/additive", "additive")?;
    assert_source_bytes(additive, 1, &STORED_FLAG.to_le_bytes(), &dba)?;
    assert_unread(&report, &dba)?;
    let ordinary = directory.path().join("ordinary.glb");
    command(&input, Some(SKIN), &ordinary, false)?;
    let bytes = fs::read(&output)?;
    assert_eq!(bytes, fs::read(&ordinary)?, "ordinary GLB bytes");
    assert_eq!(
        animation_names(&check_glb(&bytes)?),
        ["same"],
        "only matching tracks"
    );
    Ok(())
}

#[test]
fn loose_glb_animation_origins_remain_explicitly_unknown() -> TestResult {
    let directory = tempfile::tempdir()?;
    let model = TestModel::two_bone_skin();
    let input = directory.path().join("skin.skin");
    fs::write(&input, model.cgf())?;
    fs::write(directory.path().join("skin.skinm"), model.cgfm())?;
    fs::write(
        directory.path().join("same.caf"),
        rotating("same.caf", tip(), 30).caf(),
    )?;
    fs::write(
        directory.path().join("add.caf"),
        TestAnimation::new("add.caf", 30).additive().caf(),
    )?;
    let result = command(&input, None, &directory.path().join("loose.glb"), true)?;
    let report = serde_json::from_slice::<Value>(&result)?;
    for (row, fields) in [
        ("/animations/0", ["name", "duration_seconds", "additive"]),
        ("/unread_animations/0", ["source", "name", "reason"]),
    ] {
        for field in fields {
            assert_unknown(&report, &format!("{row}/{field}"))?;
        }
    }
    Ok(())
}

fn assert_baked_origins_unknown(report: &Value) -> TestResult {
    for row in 0..2 {
        for field in ["channels", "keys", "skipped_tracks"] {
            assert_unknown(report, &format!("/animations/{row}/{field}"))?;
        }
        let weights = format!("/animations/{row}/skipped_weights");
        assert!(
            report.pointer(&weights).is_none(),
            "zero skipped weights are omitted"
        );
        assert!(
            report["fieldOrigins"].get(&weights).is_none(),
            "no origin for omitted weights"
        );
    }
    Ok(())
}

fn fixture(input: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let model = TestModel::two_bone_skin();
    let unsupported = TestAnimation::new("fixture/unread.caf", 30).track(
        TestTrack::new(1)
            .rotations(&[(0, IDENTITY)])
            .rotation_code(0xff),
    );
    let dba = TestDba::new()
        .animation(unsupported)
        .animation(rotating(DBA_CLIP, tip(), 60))
        .animation(TestAnimation::new("fixture/additive.caf", 30).additive())
        .pool_after_metadata()
        .bytes();
    let failed = TestChunkFile::ivo()
        .chunk(
            CAF_DATA,
            CAF_DATA_VERSION,
            rotating("failed.caf", tip(), 30).caf_data(),
        )
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(SKIN, model.cgf()))
        .entry(TestEntry::zstd(SKIN_GEOMETRY, model.cgfm()))
        .entry(TestEntry::zstd(CAF, rotating("same.caf", 0xFEED, 15).caf()))
        .entry(TestEntry::zstd(DBA, dba.clone()).encrypted())
        .entry(TestEntry::zstd(FAILED, failed).encrypted())
        .write_to(input)?;
    Ok(dba)
}

fn command(
    input: &Path,
    entry: Option<&str>,
    output: &Path,
    as_json: bool,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    if as_json {
        command.arg("--json");
    }
    if entry.is_some() {
        command.args(["--key", &test_key().to_hex()]);
    }
    command.args(["model", "glb"]).arg(input);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let result = command
        .args(["--anim", "all", "--out"])
        .arg(output)
        .output()?;
    assert!(result.status.success(), "animation export: {result:?}");
    Ok(result.stdout)
}

fn assert_unread(report: &Value, dba: &[u8]) -> TestResult {
    assert_eq!(
        report.pointer("/unread_animations/0/source"),
        Some(&json!(FAILED)),
        "the failed file is the first unread row"
    );
    assert_eq!(
        report.pointer("/unread_animations/1/name"),
        Some(&json!("unread")),
        "the rejected database clip keeps its original name"
    );
    assert_eq!(
        report.pointer("/unread_animations/2/name"),
        Some(&json!("additive")),
        "the additive database clip is reported separately"
    );
    assert_entry(report, "/unread_animations/0/source", FAILED, false)?;
    assert_unknown(report, "/unread_animations/0/reason")?;
    assert_unknown(report, "/unread_animations/1/reason")?;
    assert_entry(report, "/unread_animations/1/name", DBA, true)?;
    assert_entry(report, "/unread_animations/2/reason", DBA, true)?;
    let rejected_name = clip_location(report, "/unread_animations/1/name", "databasePath")?;
    assert_source_bytes(rejected_name, 0, b"fixture/unread.caf", dba)?;
    let rejection = clip_location(report, "/unread_animations/2/reason", "additive")?;
    assert_source_bytes(
        rejection,
        2,
        &(STORED_FLAG | ADDITIVE_FLAG).to_le_bytes(),
        dba,
    )?;
    let attempts = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let attempted = attempts
        .iter()
        .find(|site| site["role"] == "attempted")
        .ok_or("attempted read")?;
    assert_eq!(
        entry_path(report, attempted)?,
        FAILED,
        "failed physical entry"
    );
    Ok(())
}

fn assert_unknown(report: &Value, path: &str) -> TestResult {
    let origin = field_origin(report, path)?;
    assert_eq!(origin["containsEncryptedSource"], Value::Null, "{path}");
    assert_eq!(origin["sourceSetComplete"], false, "{path}");
    assert!(field_uses(report, path)?.is_empty(), "{path}");
    Ok(())
}

fn assert_entry(report: &Value, path: &str, entry: &str, encrypted: bool) -> TestResult {
    let origin = field_origin(report, path)?;
    assert_eq!(origin["containsEncryptedSource"], encrypted, "{path}");
    let uses = field_uses(report, path)?;
    assert!(!uses.is_empty(), "observed source at {path}");
    for source_use in uses {
        assert_eq!(entry_path(report, source_use)?, entry, "{path}");
    }
    Ok(())
}

fn clip_location<'a>(
    report: &'a Value,
    path: &str,
    field: &str,
) -> Result<&'a Value, Box<dyn Error>> {
    field_uses(report, path)?
        .into_iter()
        .filter_map(|site| site.get("location"))
        .find(|location| {
            location["namespace"] == "cryEngineAnimation" && location["field"] == field
        })
        .ok_or_else(|| format!("missing original clip field {field} at {path}").into())
}

fn assert_source_bytes(
    location: &Value,
    ordinal: usize,
    expected: &[u8],
    file: &[u8],
) -> TestResult {
    let indices = location["indices"].as_array().ok_or("clip coordinates")?;
    let position = |index: usize| -> Result<usize, Box<dyn Error>> {
        Ok(indices
            .get(index)
            .and_then(Value::as_str)
            .ok_or("position")?
            .parse()?)
    };
    assert_eq!(position(0)?, ordinal, "physical clip ordinal");
    let bytes = file.get(position(2)?..position(3)?).ok_or("source bytes")?;
    assert!(bytes.starts_with(expected), "observed bytes: {bytes:?}");
    Ok(())
}
