// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error as _;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, ConflictPolicy, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{CONTAINER, Origins, Owner, TestResult};

const CANDIDATE: &str = "audio-loose-wem-candidate";
const INVENTORY: &str = "audio-loose-wem-inventory";
const LOCAL: &str = "Data/Sounds/Wwise/German/00042.WEM";
const NEUTRAL: &str = "Data/Sounds/wwise/Media/42.wem";
const NESTED: &str = "Data/Sounds/wwise/Media/English(US)/42.wem";
const EMPTY: &str = "empty.socpak";
const RAW_PATH: &[u8] = b"Data/Sounds/wwise/Fr\xff/42.wem";
const ZIP_END_BYTES: usize = 22;

#[test]
fn loose_wem_inventory_streams_all_physical_candidates_per_opened_scope() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let output = folder.path().join("media");
    let mut options = audio_options()?;
    options.limits.reference_candidates = 1;
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    assert_eq!(
        index.source_entries.written, 0,
        "all payloads were filtered"
    );
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let origins = Origins::read(&output.join(&index.artifact_base))?;
    check_scope(&rows, None, 4, true)?;
    check_scope(&rows, Some(CONTAINER), 1, true)?;
    check_scope(&rows, Some(EMPTY), 0, true)?;
    check_root_candidates(&rows, &origins)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "actual loose inventory validates"
        );
    }
    Ok(())
}

#[test]
fn a_recovered_loose_wem_scope_never_proves_a_missing_candidate() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored(NEUTRAL, b"candidate"))
        .build();
    bytes.truncate(bytes.len().saturating_sub(ZIP_END_BYTES));
    fs::write(&source, bytes)?;
    let archive = Archive::open_with(source, OpenOptions::new().recover(true))?;
    let output = folder.path().join("media");
    let mut options = audio_options()?;
    options.diagnostic = true;
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("a recovered directory published a complete inventory".into());
    };
    let root = diagnostic.parent().ok_or("diagnostic generation root")?;
    let rows = support::records(root, &index, "audio.jsonl")?;
    check_scope(&rows, None, 1, false)
}

#[test]
fn loose_wem_inventory_obeys_explicit_generation_budgets() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let output = folder.path().join("media");
    let options = audio_options()?;
    support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let before = fs::read(output.join("media-index.json"))?;
    for budget in ["sources", "metadata_work", "records"] {
        let mut restricted = options.clone();
        restricted.conflict = ConflictPolicy::Overwrite;
        match budget {
            "sources" => restricted.limits.sources = 1,
            "metadata_work" => restricted.limits.metadata_work = 1,
            _ => restricted.limits.records = 1,
        }
        let error = p4kx_media::export(&archive, &output, &restricted)
            .err()
            .ok_or("exhausted inventory published")?;
        let mut current = &error;
        while let Some(inner) = current
            .source()
            .and_then(|error| error.downcast_ref::<MediaError>())
        {
            current = inner;
        }
        assert!(
            matches!(current, MediaError::Limit { budget: actual, limit: 1 } if *actual == budget),
            "the original {budget} limit is retained, got {current}"
        );
        assert_eq!(
            fs::read(output.join("media-index.json"))?,
            before,
            "{budget} exhaustion preserves the last complete index"
        );
    }
    Ok(())
}

fn fixture(root: &Path) -> TestResult<Archive> {
    let path = root.join("Data.p4k");
    let nested = TestContainer::new()
        .entry(TestEntry::stored(NESTED, b"not read"))
        .build();
    let empty = TestContainer::new().build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(LOCAL, b"not read"))
        .entry(TestEntry::stored(LOCAL, b"encrypted").encrypted())
        .entry(TestEntry::stored(NEUTRAL, b"not read"))
        .entry(TestEntry::stored("original", b"not read").with_path_bytes(RAW_PATH))
        .entry(TestEntry::stored(
            "Data/Sounds/wwise/German/extra/42.wem",
            b"not read",
        ))
        .entry(TestEntry::stored(CONTAINER, nested))
        .entry(TestEntry::stored(EMPTY, empty))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn audio_options() -> TestResult<ExportOptions> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Audio],
        include: Some(Filter::parse("*.bnk")?),
        ..ExportOptions::default()
    })
}

fn check_scope(
    rows: &[MediaRecord],
    scope: Option<&str>,
    count: usize,
    complete: bool,
) -> TestResult {
    let rows: Vec<_> = rows.iter().filter(|row| owns_scope(row, scope)).collect();
    let summary = rows
        .iter()
        .find(|row| row.subresource.namespace == INVENTORY)
        .ok_or("scope inventory closure")?;
    assert_eq!(
        summary.data,
        json!({
            "record_type":"loose-wem-inventory",
            "payload":{"observed_candidates":count.to_string(),"candidates_complete":complete}
        }),
        "the closure reports the observed scope coverage"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.subresource.namespace == CANDIDATE)
            .count(),
        count,
        "the inventory count matches the streamed candidate occurrences"
    );
    Ok(())
}

fn owns_scope(row: &MediaRecord, scope: Option<&str>) -> bool {
    match (&row.source.identity, scope) {
        (Identity::Root, None) => true,
        (Identity::Entry { entry, containers }, Some(path)) => {
            entry.path == path && containers.is_empty()
        }
        _ => false,
    }
}

fn check_root_candidates(rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let candidates = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| owns_scope(row, None) && row.subresource.namespace == CANDIDATE);
    for (entry, (ordinal, row)) in candidates.enumerate() {
        let payload = row.data.get("payload").ok_or("typed candidate")?;
        assert_eq!(
            payload.get("media_id"),
            Some(&json!(42)),
            "the original numeric WEM stem supplies the Short ID"
        );
        assert_eq!(
            payload.pointer("/entry/entry_index"),
            Some(&json!(entry)),
            "same-name candidates keep separate original Entry identities"
        );
        assert!(
            row.source.raw.is_none(),
            "directory inventories make no payload claim"
        );
        check_placement(payload, entry)?;
        if entry == 1 {
            let evidence = origins.field("records", ordinal, "/data/payload/media_id", false)?;
            let inputs = origins.require_owner(
                evidence,
                Owner {
                    path: LOCAL,
                    entry,
                    parent: None,
                    stored: "encrypted",
                    metadata: false,
                    payload: true,
                },
            )?;
            assert!(
                inputs
                    .iter()
                    .any(|usage| usage.get("scope") == Some(&json!("archiveDirectoryMetadata"))),
                "the ID originates in this encrypted Entry's public directory"
            );
        }
    }
    Ok(())
}

fn check_placement(payload: &Value, entry: usize) -> TestResult {
    let placement = match entry {
        0 | 1 => json!({"type":"language","folder":"German"}),
        2 => json!({"type":"neutral"}),
        3 => json!({"type":"unavailable"}),
        _ => return Err("unexpected fixture candidate".into()),
    };
    assert_eq!(
        payload.get("placement"),
        Some(&placement),
        "observed folder state"
    );
    let status = if entry == 3 {
        "unavailable"
    } else {
        "not-selected"
    };
    assert_eq!(
        payload.pointer("/selection/status"),
        Some(&json!(status)),
        "scope selection and lossy path facts are distinct from payload readability"
    );
    let access = if entry == 1 {
        "key-required"
    } else {
        "not-inspected"
    };
    assert_eq!(
        payload.get("payload_access"),
        Some(&json!(access)),
        "payload admission evidence"
    );
    if entry == 3 {
        assert!(
            payload
                .pointer("/entry/raw_path_hex")
                .and_then(Value::as_str)
                .is_some(),
            "the physical non-UTF-8 path remains available"
        );
    }
    Ok(())
}
