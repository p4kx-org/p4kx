// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Exclusion, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAnimation, TestDba, TestEntity, TestEntry, TestLoadout,
    TestLoadoutEntry, TestModel, TestSkeleton, TestTrack, entity_schema, test_key,
};
use serde_json::Value;

use super::support::{TestResult, published, records};

const CAF: &str = "Data/Animations/Fixture/sub/clip.caf";
const OMITTED: &str = "Data/Animations/Fixture/omitted.caf";
const DBA: &str = "Data/Animations/Banks/mixed.dba";

#[test]
fn real_generation_joins_placements_to_physical_clips_and_preserves_filter_and_origin() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), false)?;
    let output = scratch.path().join("output");
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        exclude: Exclusion::parse([OMITTED])?,
        animation_entity: Some("FixtureRoot".into()),
        ..ExportOptions::default()
    };
    let index = published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = records(&output, &index, "animation.jsonl")?;
    verify_shape(&rows)?;
    verify_clip_origins(&output.join(&index.artifact_base), &rows)?;
    Ok(())
}

#[test]
fn filtered_owner_and_unavailable_xml_do_not_publish_a_complete_catalogue() -> TestResult {
    let scratch = tempfile::tempdir()?;
    for (slot, filter, expected) in [
        (0, Some("Data/Animations/*"), "dataforge-not-selected"),
        (1, None, "incomplete-loadout"),
    ] {
        let archive = fixture(scratch.path(), true)?;
        let output = scratch.path().join(format!("output-{slot}"));
        let options = ExportOptions {
            kinds: vec![MediaKind::Animation],
            include: filter.map(Filter::parse).transpose()?,
            exclude: Exclusion::parse([OMITTED])?,
            diagnostic: true,
            animation_entity: Some("FixtureRoot".into()),
            ..ExportOptions::default()
        };
        let ExportOutcome::Incomplete { diagnostic, .. } =
            p4kx_media::export(&archive, &output, &options)?
        else {
            return Err("limited entity scope must remain diagnostic".into());
        };
        assert!(!output.join("media-index.json").exists());
        let lines = fs::read_to_string(
            diagnostic
                .parent()
                .ok_or("generation")?
                .join("animation.jsonl"),
        )?;
        let owner = lines
            .lines()
            .map(serde_json::from_str::<MediaRecord>)
            .collect::<Result<Vec<_>, _>>()?;
        let states: Vec<_> = owner
            .iter()
            .filter(|row| row.subresource.namespace == "animation-entity-catalogue")
            .filter_map(|row| row.data.pointer("/payload/state").and_then(Value::as_str))
            .collect();
        assert_eq!(states, [expected]);
    }
    Ok(())
}

#[test]
fn stopped_or_budget_refused_entity_projection_leaves_the_current_index_unpublished() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), false)?;
    let output = scratch.path().join("refused");
    let mut options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        animation_entity: Some("FixtureRoot".into()),
        exclude: Exclusion::parse([OMITTED])?,
        ..ExportOptions::default()
    };
    options.limits.metadata_work = 5;
    assert!(matches!(
        p4kx_media::export(&archive, &output, &options),
        Err(error) if error.to_string().contains("metadata_work")
    ));
    assert!(!output.join("media-index.json").exists());

    options.limits = p4kx_media::Limits::default();
    options.cancel.cancel();
    assert!(matches!(
        p4kx_media::export(&archive, &output, &options),
        Err(p4kx_media::MediaError::Cancelled)
    ));
    assert!(!output.join("media-index.json").exists());
    Ok(())
}

fn verify_shape(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    let rig_states: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-entity-rig")
        .filter_map(|row| {
            row.data
                .pointer("/payload/skeleton/status")
                .and_then(Value::as_str)
        })
        .collect();
    assert_eq!(
        rig_states,
        ["missing", "read"],
        "independent records retain a missing root"
    );
    let omitted = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-source"
                && row.data.pointer("/payload/path").and_then(Value::as_str) == Some(OMITTED)
        })
        .ok_or("filtered physical candidate")?;
    assert_eq!(
        omitted
            .data
            .pointer("/payload/status")
            .and_then(Value::as_str),
        Some("not-selected"),
        "excluded source identity survives without payload"
    );
    verify_physical_refs(rows)
}

fn verify_physical_refs(rows: &[MediaRecord]) -> TestResult {
    let clips: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-entity-clip")
        .collect();
    assert_eq!(
        clips.len(),
        3,
        "one loose CAF and both independent DBA ordinals"
    );
    for (row, rate) in clips.into_iter().zip([24, 30, 60]) {
        assert_eq!(
            row.data
                .pointer("/payload/frames_per_second")
                .and_then(Value::as_u64),
            Some(rate),
            "same-name physical clips retain their original timing"
        );
        let physical = row
            .data
            .pointer("/payload/physical_record")
            .ok_or("physical clip reference")?;
        let source = physical.get("source").ok_or("reference Entry identity")?;
        let subresource = physical.get("subresource").ok_or("physical clip ordinal")?;
        assert!(
            rows.iter()
                .any(|row| row.subresource.namespace == "animation-clip"
                    && serde_json::to_value(&row.source.identity)
                        .is_ok_and(|value| value == *source)
                    && serde_json::to_value(&row.subresource)
                        .is_ok_and(|value| value == *subresource)),
            "entity clip points to an actual ordinary animation row"
        );
    }
    Ok(())
}

fn verify_clip_origins(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let origins = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let fields = origins
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    for (ordinal, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-entity-clip")
    {
        let ordinal = ordinal.to_string();
        let rate = row
            .data
            .pointer("/payload/frames_per_second")
            .and_then(Value::as_u64);
        let proof = |address| {
            fields
                .iter()
                .find(|field| {
                    field.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("animation")
                        && field.pointer("/scope/ordinal").and_then(Value::as_str)
                            == Some(ordinal.as_str())
                        && field.get("field").and_then(Value::as_str) == Some(address)
                })
                .ok_or("immediate per-field entity clip origin")
        };
        for (address, encrypted) in [
            ("/data/payload/frames_per_second", rate == Some(24)),
            ("/data/payload/physical_record/source/entry/path", false),
        ] {
            let field = proof(address)?;
            assert_eq!(
                field
                    .get("containsEncryptedSource")
                    .and_then(Value::as_bool),
                Some(encrypted),
                "{address} retains its actual payload or directory scope",
            );
            assert_eq!(
                field.get("sourceSetComplete").and_then(Value::as_bool),
                Some(true),
                "{address} immediate source is fully observed",
            );
        }
    }
    Ok(())
}

fn fixture(folder: &Path, external_loadout: bool) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            database(external_loadout),
        ))
        .entry(TestEntry::stored(
            "Data/Objects/Fixture/child.chr",
            TestModel::cube()
                .with_skeleton(&TestSkeleton::two_bones())
                .cgf(),
        ));
    for (path, params) in [
        (
            "Data/Objects/Fixture/root.chrparams",
            r##"<Params><AnimationList><Animation name="#filepath" path="Animations/Fixture"/></AnimationList></Params>"##,
        ),
        (
            "Data/Objects/Fixture/child.chrparams",
            r#"<Params><AnimationList><Animation name="$TracksDatabase" path="Animations/Banks/mixed.dba"/></AnimationList></Params>"#,
        ),
    ] {
        builder = builder.entry(TestEntry::stored(path, params));
    }
    builder
        .entry(TestEntry::stored(CAF, animation(24).caf()).encrypted())
        .entry(TestEntry::stored(OMITTED, animation(50).caf()).encrypted())
        .entry(TestEntry::stored(
            DBA,
            TestDba::new()
                .animation(animation(30))
                .animation(animation(60))
                .bytes(),
        ))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn database(external_loadout: bool) -> Vec<u8> {
    let loadout = if external_loadout {
        TestLoadout::Xml("Objects/Fixture/external.xml".into())
    } else {
        TestLoadout::Manual(vec![TestLoadoutEntry::class("port", "FixtureChild")])
    };
    let root = TestEntity::new(1, "FixtureRoot", "entities/fixture_root.xml")
        .geometry("Objects/Fixture/root.chr", None)
        .loadout(loadout);
    let child = TestEntity::new(2, "FixtureChild", "entities/fixture_child.xml")
        .geometry("Objects/Fixture/child.chr", None);
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .build()
}

fn animation(rate: u16) -> TestAnimation {
    TestAnimation::new("same", rate)
        .track(TestTrack::new(41).positions(&[(0, [0.0, 0.0, 0.0]), (12, [1.0, 0.0, 0.0])]))
}
