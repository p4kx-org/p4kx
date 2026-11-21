// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::f32::consts::FRAC_1_SQRT_2;
use std::fs;
use std::path::Path;

use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBone, TestChunkFile, TestEntry, TestSkeleton, test_key};
use serde::Deserialize;
use serde_json::{Value, json};

use super::support::{self, TestResult};

#[path = "model_cdf_model_pose.rs"]
mod model_pose;

const RIG_SOURCE: &str = "Rigs/SelectionRig.chr";
const DECOY_SOURCE: &str = "Rigs/Unselected.chr";
const ATTACHMENT: &str = "model-character-attachment";
const PLACEMENT: &str = "/payload/rig_placement";
const FRAME: &str = "/data/payload/rig_placement/rig_from_attachment";
const RIGGED_CDF: &str = r#"<CharacterDefinition>
    <Model File="Rigs/SelectionRig.chr"/>
    <AttachmentList>
        <Attachment AName="Bound" Type="CA_BONE" BoneName="JOINT" Binding="Rigs/Absent.cgf" RelPosition="2,0,0" RelRotation="0,0,0,1"/>
        <Attachment AName="Ambiguous" Type="CA_BONE" BoneName="SHARED" Binding="Rigs/Absent.cgf"/>
        <Attachment AName="Missing" Type="CA_BONE" BoneName="AbsentJoint" Binding="Rigs/Absent.cgf"/>
        <Attachment AName="Invalid" Type="CA_BONE" BoneName="Joint" Binding="Rigs/Absent.cgf" RelRotation="0,0,0,0"/>
        <Attachment AName="Unnamed" Type="CA_BONE" Binding="Rigs/Absent.cgf"/>
        <Attachment AName="Face" Type="CA_FACE" BoneName="Joint"/>
        <Attachment AName="Model" Type="CA_BONE" BoneName="Joint" Position="100,200,300" Rotation="1,0,0,0"/>
        <Attachment AName="Mixed" Type="CA_BONE" BoneName="Joint" Position="2,3,4" RelRotation="1,0,0,0" Rotation="invalid shadowed"/>
        <Attachment AName="Default" Type="CA_BONE" BoneName="Joint" RelRotation="1,0,0,0" Rotation="presence selects model default"/>
    </AttachmentList>
</CharacterDefinition>"#;
const UNSELECTED_CDF: &str = r#"<CharacterDefinition>
    <Model File="Rigs/Unselected.chr"/>
    <AttachmentList>
        <Attachment Type="CA_BONE" BoneName="Joint" Binding="Rigs/Absent.cgf" RelPosition="1,2,3"/>
    </AttachmentList>
</CharacterDefinition>"#;

#[test]
fn cdf_rig_placement_binds_only_the_selected_physical_skeleton_and_unique_bone() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let fixture = fixture(scratch.path())?;
    let output = scratch.path().join("out");
    let index = support::published(p4kx_media::export(
        &fixture.archive,
        &output,
        &ExportOptions {
            kinds: vec![MediaKind::Model],
            include: Some(Filter::parse("*Selection*")?),
            ..ExportOptions::default()
        },
    )?)?;
    assert_eq!(
        index.source_entries.written, 3,
        "two CDFs and the selected rig are read"
    );
    let rows = support::records(&output, &index, "model.jsonl")?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        schema
            .validate(&serde_json::to_value(row)?)
            .map_err(|cause| cause.to_string())?;
    }
    let (ordinal, bound) = attachment(&rows, 0, 0)?;
    assert_bound(bound)?;
    assert_unapplied(&rows)?;
    let base = output.join(&index.artifact_base);
    check_frame_evidence(&base, ordinal, fixture.skeleton_offset)?;
    model_pose::check(&rows, &base)
}

struct Fixture {
    archive: Archive,
    skeleton_offset: u64,
}

fn fixture(folder: &Path) -> Result<Fixture, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let rig = TestChunkFile::ivo().chunk(
        ivo::COMPILED_BONES_2.id(),
        0x900,
        skeleton().compiled_bones_2(),
    );
    let skeleton_offset = rig
        .offsets()
        .first()
        .copied()
        .ok_or("physical skeleton extent")?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Rigs/SelectionCraft.cdf",
            RIGGED_CDF.as_bytes(),
        ))
        .entry(TestEntry::zstd(format!("Data/{RIG_SOURCE}"), rig.bytes()).encrypted())
        .entry(TestEntry::stored(
            "Data/Rigs/SelectionOther.cdf",
            UNSELECTED_CDF.as_bytes(),
        ))
        .entry(TestEntry::stored(format!("Data/{DECOY_SOURCE}"), b"unread decoy").with_crc32c(1))
        .write_to(&path)?;
    Ok(Fixture {
        archive: Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?,
        skeleton_offset,
    })
}

fn skeleton() -> TestSkeleton {
    const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    TestSkeleton::new(vec![
        TestBone {
            name: "Root".to_owned(),
            parent: None,
            rotation: [0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2],
            translation: [10.0, 20.0, 30.0],
        },
        TestBone {
            name: "Pivot".to_owned(),
            parent: Some(0),
            rotation: IDENTITY,
            translation: [2.0, 0.0, 0.0],
        },
        TestBone {
            name: "Joint".to_owned(),
            parent: Some(1),
            rotation: IDENTITY,
            translation: [1.0, 0.0, 0.0],
        },
        TestBone {
            name: "Shared".to_owned(),
            parent: Some(0),
            rotation: IDENTITY,
            translation: [0.0; 3],
        },
        TestBone {
            name: "shared".to_owned(),
            parent: Some(1),
            rotation: IDENTITY,
            translation: [0.0; 3],
        },
    ])
}

fn attachment(
    rows: &[MediaRecord],
    entry_index: usize,
    child: u64,
) -> Result<(usize, &MediaRecord), Box<dyn Error>> {
    rows.iter().enumerate().find(|(_, row)| {
        matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == entry_index)
            && row.subresource.namespace == ATTACHMENT && row.subresource.indices == [1, child]
    }).ok_or_else(|| format!("attachment {child} in Entry {entry_index}").into())
}

fn placement(row: &MediaRecord) -> Result<&Value, Box<dyn Error>> {
    row.data
        .pointer(PLACEMENT)
        .ok_or_else(|| "rig placement result".into())
}

fn assert_bound(row: &MediaRecord) -> TestResult {
    let result = placement(row)?;
    assert_eq!(
        result["disposition"], "bound",
        "the unique original bone binds"
    );
    assert_eq!(
        result["skeleton_chunk"], 0,
        "the original compiled skeleton ordinal"
    );
    assert_eq!(result["bone_ordinals"], json!([2]), "one original bone");
    assert_eq!(
        result.pointer("/rig/original/value"),
        Some(&json!(RIG_SOURCE)),
        "the original rig file spelling"
    );
    assert_eq!(
        result.pointer("/rig/resolution/target/source/entry/entry_index"),
        Some(&json!(1)),
        "the chosen physical rig Entry"
    );
    assert_eq!(
        row.data.pointer("/payload/binding/resolution/status"),
        Some(&json!("missing")),
        "the geometry target is resolved independently"
    );
    let frame = result.get("rig_from_attachment").ok_or("rig-space frame")?;
    let matrix = <[[f64; 4]; 3]>::deserialize(frame)?;
    let expected = [
        [0.0, 1.0, 0.0, 10.0],
        [-1.0, 0.0, 0.0, 25.0],
        [0.0, 0.0, 1.0, 30.0],
    ];
    for (actual, expected) in matrix
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert!(
            (actual - expected).abs() < 1e-5,
            "rig frame component {actual} versus {expected}"
        );
    }
    Ok(())
}

fn assert_unapplied(rows: &[MediaRecord]) -> TestResult {
    for (child, disposition, bones) in [
        (1, "ambiguous-bone", json!([3, 4])),
        (2, "missing-bone", json!([])),
        (3, "invalid-offset", json!([2])),
        (4, "bone-name-missing", json!([])),
    ] {
        let result = placement(attachment(rows, 0, child)?.1)?;
        assert_eq!(
            result["disposition"], disposition,
            "physical attachment {child}"
        );
        assert_eq!(result["bone_ordinals"], bones, "actual original candidates");
        assert_eq!(
            result.get("rig_from_attachment"),
            Some(&Value::Null),
            "no unapplied frame"
        );
    }
    let face = attachment(rows, 0, 5)?.1;
    assert_eq!(
        face.data.pointer(PLACEMENT),
        None,
        "face attachment has no bone placement"
    );
    let excluded = placement(attachment(rows, 2, 0)?.1)?;
    assert_eq!(
        excluded["disposition"], "target-not-selected",
        "decoy rig is not read"
    );
    assert_eq!(
        excluded.pointer("/rig/original/value"),
        Some(&json!(DECOY_SOURCE)),
        "the locator survives"
    );
    assert_eq!(
        excluded.pointer("/rig/resolution/target/source/entry/entry_index"),
        Some(&json!(3)),
        "physical target is known from the directory"
    );
    assert_eq!(
        excluded.get("rig_from_attachment"),
        Some(&Value::Null),
        "no frame from an unselected rig"
    );
    Ok(())
}

fn frame_origin(base: &Path, ordinal: usize) -> Result<Value, Box<dyn Error>> {
    let ordinal = ordinal.to_string();
    let frame = json_rows(base, "field-origins.jsonl")?
        .into_iter()
        .find(|row| {
            row.get("field").and_then(Value::as_str) == Some(FRAME)
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("model")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
        })
        .ok_or("frame provenance")?;
    assert_eq!(
        frame.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the encrypted rig contributes"
    );
    assert_eq!(
        frame.get("sourceSetComplete"),
        Some(&json!(true)),
        "both source parts are available"
    );
    Ok(frame)
}

fn check_frame_evidence(base: &Path, ordinal: usize, skeleton_offset: u64) -> TestResult {
    let frame = frame_origin(base, ordinal)?;
    let refs = frame
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("frame inputs")?;
    let origin_rows = json_rows(base, "origin-uses.jsonl")?;
    let inputs = json_rows(base, "origin-sources.jsonl")?;
    let mut cdf_offsets = [false; 2];
    let pose_sites = original_pose_sites(skeleton_offset);
    let mut observed = [false; 3];
    for reference in refs {
        let used = origin_rows
            .iter()
            .find(|item| item.get("id") == Some(reference))
            .ok_or("referenced use")?;
        if used.get("scope").and_then(Value::as_str) != Some("payload") {
            continue;
        }
        let source = inputs
            .iter()
            .find(|input| input.get("id") == used.get("source"))
            .ok_or("referenced Entry")?;
        match source.get("entryIndex").and_then(Value::as_u64) {
            Some(0) => {
                cdf_offsets[0] |=
                    used.pointer("/location/field").and_then(Value::as_str) == Some("RelPosition");
                cdf_offsets[1] |=
                    used.pointer("/location/field").and_then(Value::as_str) == Some("RelRotation");
            }
            Some(1) if used.get("role").and_then(Value::as_str) == Some("conversion") => {
                for (seen, expected) in observed.iter_mut().zip(&pose_sites) {
                    *seen |= used.get("location") == Some(expected);
                }
            }
            Some(1) => {}
            None if is_directory_selector(source, used) => {}
            other => {
                return Err(format!(
                    "unexpected frame payload source {other:?}: {source}; use {used}"
                )
                .into());
            }
        }
    }
    assert_eq!(
        cdf_offsets,
        [true, true],
        "both original offsets contribute"
    );
    assert_eq!(
        observed, [true; 3],
        "each original lineage pose contributes its exact span"
    );
    Ok(())
}

fn is_directory_selector(source: &Value, used: &Value) -> bool {
    source.get("kind").and_then(Value::as_str) == Some("archiveFile")
        && used.get("role").and_then(Value::as_str) == Some("selector")
        && used.pointer("/location/namespace").and_then(Value::as_str) == Some("archive-directory")
        && used.pointer("/location/field").and_then(Value::as_str) == Some("matching-inventory")
}

fn original_pose_sites(chunk: u64) -> [Value; 3] {
    const FIRST_RELATIVE_POSE: u64 = 16;
    const BONE_RECORD: u64 = 68;
    const POSE_BYTES: u64 = 28;
    [0, 1, 2].map(|bone| {
        let begin = chunk + FIRST_RELATIVE_POSE + bone * BONE_RECORD;
        json!({
            "type": "subresource",
            "namespace": "model-skeleton-bone",
            "field": "relativePose",
            "indices": ([0, bone, begin, begin + POSE_BYTES].map(|value| value.to_string())),
        })
    })
}

fn json_rows(base: &Path, file: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(base.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
