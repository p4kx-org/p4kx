// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, ConflictPolicy, Exclusion, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, TestSkeleton, TestTrack,
    test_key,
};
use serde_json::{Value, json};

use super::{STREAM, TestResult, assert_value, find, options, support, validate};

const PATH: &str = "rig.chr";
const SOURCE: &str = "animation-skeleton";
const BINDING: &str = "animation-controller-binding";

fn skeleton() -> TestResult<Vec<u8>> {
    let mut data = TestSkeleton::chain(3).compiled_bones_2();
    data.get_mut(72..76)
        .ok_or("second physical ID")?
        .copy_from_slice(&TestSkeleton::controller_id(0).to_le_bytes());
    Ok(TestChunkFile::ivo().chunk(0xC201_1111, 1, data).bytes())
}

fn clip() -> TestAnimation {
    let id = TestSkeleton::controller_id(0);
    let track = |id| TestTrack::new(id).positions(&[(0, [0.0; 3]), (3, [0.0, 2.0, 0.0])]);
    TestAnimation::new("clip.caf", 30)
        .additive()
        .track(TestTrack::new(id))
        .track(track(id))
        .track(track(id))
        .track(track(17))
}

fn export_options() -> TestResult<ExportOptions> {
    Ok(ExportOptions {
        include: Some(Filter::parse("*clip*")?),
        animation_skeleton: Some(PATH.to_owned()),
        ..options()
    })
}

#[test]
fn explicit_binding_keeps_every_caf_and_dba_track_and_limits_skeleton_origin_to_binding()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let animation = clip();
    let caf = animation.caf();
    let dba = TestDba::new().animation(animation).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("clip.caf", caf))
        .entry(TestEntry::stored("clip.dba", dba))
        .entry(TestEntry::zstd(PATH, skeleton()?).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = folder.path().join("bound");
    let opts = export_options()?;
    let index = support::published(p4kx_media::export(&archive, &output, &opts)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    validate(&rows)?;
    let base = output.join(&index.artifact_base);
    let origins = Origins::read(&base)?;
    for owner in [0, 1] {
        check_bindings(&rows, owner, &origins)?;
    }
    let retained = json_rows(&base, "sources.jsonl")?;
    assert_eq!(
        retained
            .iter()
            .filter(|row| row.pointer("/identity/entry/path") == Some(&json!(PATH)))
            .count(),
        1,
        "the include-filtered skeleton is retained once"
    );
    let raw_output = folder.path().join("raw");
    let raw = support::published(p4kx_media::export(
        &archive,
        &raw_output,
        &ExportOptions {
            animation_skeleton: None,
            ..opts
        },
    )?)?;
    let raw_rows = support::records(&raw_output, &raw, STREAM)?;
    assert_eq!(
        rows.iter()
            .filter(|row| ![SOURCE, BINDING].contains(&row.subresource.namespace.as_str()))
            .map(|row| &row.data)
            .collect::<Vec<_>>(),
        raw_rows.iter().map(|row| &row.data).collect::<Vec<_>>(),
        "all old authored rows and sample artifact references are unchanged"
    );
    Ok(())
}

fn check_bindings(rows: &[MediaRecord], owner: usize, origins: &Origins) -> TestResult {
    let (_, selected) = find(rows, owner, SOURCE, &[])?;
    assert_value(&selected.data, "/payload/skeleton/state", &json!("ready"));
    assert_value(&selected.data, "/payload/skeleton/bone_count", &json!(3));
    assert_value(
        &selected.data,
        "/payload/reference/resolution/status",
        &json!("not-selected"),
    );
    for (controller, state, first, eligible, bone, count, bound) in [
        (0, "empty-controller", 0, None, Some(0), 2, None),
        (1, "bound", 0, Some(1), Some(0), 2, Some(0)),
        (2, "duplicate-controller", 0, Some(1), Some(0), 2, None),
        (3, "missing-bone", 3, Some(3), None, 0, None),
    ] {
        let (_, row) = find(rows, owner, BINDING, &[0, controller])?;
        assert_value(
            &row.data,
            "/payload",
            &json!({
                "disposition":state, "first_controller_ordinal":first, "first_track_ordinal":eligible,
                "first_bone_ordinal":bone, "matching_bones":count, "bound_bone_ordinal":bound,
            }),
        );
    }
    check_field_origins(rows, owner, origins)
}

fn check_field_origins(rows: &[MediaRecord], owner: usize, origins: &Origins) -> TestResult {
    let (binding, _) = find(rows, owner, BINDING, &[0, 1])?;
    let (samples, _) = find(rows, owner, "animation-controller", &[0, 1])?;
    let (duplicate, _) = find(rows, owner, BINDING, &[0, 2])?;
    origins.assert_input(
        duplicate,
        "/data/payload/bound_bone_ordinal",
        false,
        &[owner],
    )?;
    origins.assert_input(duplicate, "/data/payload/disposition", true, &[owner, 2])?;
    origins.assert_input(
        binding,
        "/data/payload/first_controller_ordinal",
        false,
        &[owner],
    )?;
    origins.assert_input(
        binding,
        "/data/payload/first_track_ordinal",
        false,
        &[owner],
    )?;
    origins.assert_input(
        binding,
        "/data/payload/bound_bone_ordinal",
        true,
        &[owner, 2],
    )?;
    origins.assert_input(
        samples,
        "/data/payload/translation/samples/frames/artifact/digest/hex",
        false,
        &[owner],
    )
}

#[test]
fn missing_duplicate_and_excluded_skeletons_have_distinct_dispositions() -> TestResult {
    let bytes = skeleton()?;
    let directory = tempfile::tempdir()?;
    for (state, reference, entries, exclude) in [
        ("missing", "missing", Vec::new(), false),
        (
            "ambiguous",
            "ambiguous",
            vec![
                TestEntry::stored(PATH, bytes.as_slice()),
                TestEntry::zstd(PATH, bytes.as_slice()).encrypted(),
            ],
            false,
        ),
        (
            "explicitly-excluded",
            "not-selected",
            vec![TestEntry::zstd(PATH, bytes.as_slice()).encrypted()],
            true,
        ),
    ] {
        let folder = tempfile::tempdir_in(directory.path())?;
        let mut entries = entries;
        entries.insert(0, TestEntry::stored("clip.caf", clip().caf()));
        let archive = support::archive_with(folder.path(), entries)?;
        let output = folder.path().join("out");
        let mut opts = export_options()?;
        if exclude {
            opts.exclude = Exclusion::parse(&[PATH.to_owned()])?;
        }
        let index = support::published(p4kx_media::export(&archive, &output, &opts)?)?;
        let rows = support::records(&output, &index, STREAM)?;
        validate(&rows)?;
        let (_, row) = find(&rows, 0, SOURCE, &[])?;
        assert_value(&row.data, "/payload/skeleton/state", &json!(state));
        assert_value(
            &row.data,
            "/payload/reference/resolution/status",
            &json!(reference),
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == BINDING)
                .count(),
            0,
            "the unresolved skeleton supplies no bone matching"
        );
        assert!(
            find(&rows, 0, "animation-controller", &[0, 3]).is_ok(),
            "every raw controller remains"
        );
    }
    Ok(())
}

#[test]
fn skeleton_lookup_does_not_cross_the_immediately_owning_archive() -> TestResult {
    let folder = tempfile::tempdir()?;
    let child = ArchiveBuilder::new()
        .entry(TestEntry::stored("clip.caf", clip().caf()))
        .entry(TestEntry::stored(PATH, skeleton()?))
        .build();
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("clip.caf", clip().caf()),
            TestEntry::stored("clip-child.socpak", child),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &export_options()?)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    validate(&rows)?;
    let selections: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == SOURCE)
        .collect();
    assert_eq!(selections.len(), 2, "each owning archive gets a selection");
    for row in selections {
        let p4kx_media::Identity::Entry { containers, .. } = &row.source.identity else {
            return Err("expected physical clip Entry".into());
        };
        let state = if containers.is_empty() {
            "missing"
        } else {
            "ready"
        };
        assert_value(&row.data, "/payload/skeleton/state", &json!(state));
    }
    Ok(())
}

#[test]
fn a_malformed_skeleton_retains_raw_rows_in_diagnostics_and_keeps_the_previous_index() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("clip.caf", clip().caf()),
            TestEntry::stored(PATH, b"invalid container".as_slice()),
        ],
    )?;
    let output = folder.path().join("out");
    let opts = export_options()?;
    support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            animation_skeleton: None,
            ..opts.clone()
        },
    )?)?;
    let previous = fs::read(output.join("media-index.json"))?;
    let outcome = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            diagnostic: true,
            conflict: ConflictPolicy::Overwrite,
            ..opts
        },
    )?;
    let ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("malformed selected skeleton should be diagnostic".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation root")?;
    let rows: Vec<MediaRecord> = fs::read_to_string(base.join(STREAM))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    validate(&rows)?;
    let (_, row) = find(&rows, 0, SOURCE, &[])?;
    assert_value(
        &row.data,
        "/payload/skeleton/state",
        &json!("invalid-container"),
    );
    assert!(
        find(&rows, 0, "animation-controller", &[0, 3]).is_ok(),
        "raw sample metadata survives"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous,
        "the previous index is preserved"
    );
    Ok(())
}

struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    fn read(base: &Path) -> TestResult<Self> {
        Ok(Self {
            fields: json_rows(base, "field-origins.jsonl")?,
            uses: json_rows(base, "origin-uses.jsonl")?,
            sources: json_rows(base, "origin-sources.jsonl")?,
        })
    }

    fn assert_input(
        &self,
        ordinal: usize,
        field: &str,
        encrypted: bool,
        owners: &[usize],
    ) -> TestResult {
        let ordinal = ordinal.to_string();
        let row = self
            .fields
            .iter()
            .find(|row| {
                row["field"] == field
                    && row.pointer("/scope/stream/kind") == Some(&json!("animation"))
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
            })
            .ok_or("binding or sample field receipt")?;
        assert_value(row, "/containsEncryptedSource", &json!(encrypted));
        assert_value(row, "/sourceSetComplete", &json!(true));
        let mut found = Vec::new();
        for reference in row["sourceUsesRefs"]
            .as_array()
            .ok_or("source use references")?
        {
            let usage = self
                .uses
                .iter()
                .find(|usage| usage.get("id") == Some(reference))
                .ok_or("resolved source use")?;
            let source = self
                .sources
                .iter()
                .find(|source| source.get("id") == usage.get("source"))
                .ok_or("resolved physical source")?;
            let owner = source["entryIndex"]
                .as_u64()
                .ok_or("physical entry index")?;
            found.push(usize::try_from(owner)?);
        }
        found.sort_unstable();
        found.dedup();
        assert_eq!(found, owners, "exact physical contributors of {field}");
        Ok(())
    }
}

fn json_rows(base: &Path, name: &str) -> TestResult<Vec<Value>> {
    fs::read_to_string(base.join(name))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
