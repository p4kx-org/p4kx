// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Exclusion};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status};
use serde_json::{Value, json};

use super::super::animation_definition_origins::Origins;
use super::super::support::{TestResult, published, records};
use super::{BAD_DBA, CAF, DBA, FixtureLayout, OMITTED, fixture, verify_physical_refs};

#[test]
fn a_malformed_sibling_bank_retains_independent_clips_in_the_diagnostic_generation() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), FixtureLayout::SiblingBank)?;
    let output = scratch.path().join("output");
    let options = options(Exclusion::parse([OMITTED])?);
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("malformed bank must have explicit diagnostic coverage".into());
    };
    assert!(
        !output.join("media-index.json").exists(),
        "no complete generation"
    );
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = fs::read_to_string(base.join("animation.jsonl"))?
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    verify(&archive, &rows, base, "malformed")
}

#[test]
fn an_excluded_sibling_bank_has_no_payload_proof_and_preserves_the_clip_inventory() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), FixtureLayout::SiblingBank)?;
    let output = scratch.path().join("output");
    let options = options(Exclusion::parse([OMITTED, BAD_DBA])?);
    let index = published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = records(&output, &index, "animation.jsonl")?;
    let base = output.join(&index.artifact_base);
    verify(&archive, &rows, &base, "not-selected")
}

fn options(exclude: Exclusion) -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Animation],
        animation_entity: Some("FixtureRoot".into()),
        diagnostic: true,
        exclude,
        ..ExportOptions::default()
    }
}

fn verify(archive: &Archive, rows: &[MediaRecord], base: &Path, bad_status: &str) -> TestResult {
    verify_physical_refs(rows)?;
    verify_rigs(rows)?;
    verify_failed_bank(archive, rows, bad_status)?;
    let origins = Origins::read_stream(base, "animation")?;
    for (ordinal, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-entity-clip")
    {
        let rate = row
            .data
            .pointer("/payload/frames_per_second")
            .and_then(Value::as_u64);
        let (path, encrypted) = match rate {
            Some(24) => (CAF, true),
            Some(30 | 60) => (DBA, false),
            _ => return Err("an unrecognized surviving clip".into()),
        };
        let source = entry(archive, path)?;
        let entries =
            origins.entry_indices(ordinal, "/data/payload/frames_per_second", encrypted)?;
        assert_eq!(
            entries.into_iter().collect::<Vec<_>>(),
            [source],
            "the bad sibling contributes no original bytes to the surviving frame rate",
        );
    }
    Ok(())
}

fn verify_rigs(rows: &[MediaRecord]) -> TestResult {
    let rigs: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == "animation-entity-rig")
        .collect();
    let [root, bad, good] = rigs.as_slice() else {
        return Err("root, failed-bank rig and readable rig must be retained".into());
    };
    assert_eq!(
        root.data.pointer("/payload/skeleton/status"),
        Some(&json!("missing")),
        "failed root"
    );
    for (row, ordinal, port) in [(*bad, 1, "bankBad"), (*good, 2, "bankGood")] {
        assert_eq!(
            row.subresource.indices,
            [ordinal],
            "physical placement order"
        );
        assert_eq!(
            row.data.pointer("/payload/placement/port_path"),
            Some(&json!(port)),
            "owning port"
        );
        assert_eq!(
            row.data.pointer("/payload/skeleton/status"),
            Some(&json!("read")),
            "independently usable child rig"
        );
    }
    Ok(())
}

fn verify_failed_bank(archive: &Archive, rows: &[MediaRecord], status: &str) -> TestResult {
    let bank = entry(archive, BAD_DBA)?;
    let ordinary = rows.iter().find(|row| {
        row.subresource.namespace == "animation-entry"
            && matches!(&row.source.identity, Identity::Entry { entry, containers }
                if entry.entry_index == bank && containers.is_empty())
    });
    if status == "malformed" {
        assert_eq!(
            ordinary.map(|row| row.status),
            Some(Status::Partial),
            "verified input retains its own failed projection"
        );
    } else {
        assert!(
            ordinary.is_none(),
            "the excluded bank has no ordinary payload row"
        );
    }
    let failed = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-source"
                && row.data.pointer("/payload/path").and_then(Value::as_str) == Some(BAD_DBA)
        })
        .ok_or("the attempted original bank")?;
    assert_eq!(
        failed.data.pointer("/payload/status"),
        Some(&json!(status)),
        "typed failed bank status"
    );
    let [source] = failed.subresource.indices.as_slice() else {
        return Err("the selected bank must have one physical source ordinal".into());
    };
    assert!(
        rows.iter().any(
            |row| row.subresource.namespace == "animation-entity-source-link"
                && row.subresource.indices == [1, 0, 0]
                && row.data.pointer("/payload/source") == Some(&json!(source))
        ),
        "the failed bank is attached only through its own rig and parameter link",
    );
    Ok(())
}

fn entry(archive: &Archive, path: &str) -> Result<usize, Box<dyn Error>> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path().replace('\\', "/") == path)
        .map(p4kx_extraction::Entry::index)
        .ok_or_else(|| "missing original clip Entry".into())
}
