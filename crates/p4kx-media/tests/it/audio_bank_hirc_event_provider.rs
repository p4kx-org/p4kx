// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, HircLayout, OpenOptions, Soundbank};
use p4kx_media::{
    ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord, Status,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::{layout_health, support};

#[path = "audio_bank_hirc_event_evidence.rs"]
mod evidence;
#[path = "audio_bank_hirc_event_fixture.rs"]
mod fixture;

use self::evidence::Evidence;
use self::fixture::{Fixture, range};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SAME_NAME: &str = "event.BNK";
const EVENT: &str = "audio-bank-hirc-event";
const ACTION: &str = "audio-bank-hirc-event-action";
const TAIL: &str = "audio-bank-hirc-event-tail";
const AUDIO_STREAM: &str = "audio.jsonl";

#[test]
fn event_count_versions_and_duplicate_action_words_use_original_physical_ranges() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixtures = [fixture::fixed()?, fixture::variable()?];
    for original in &fixtures {
        original.check_legacy()?;
    }
    let archive = archive(folder.path(), &fixtures)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let base = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, AUDIO_STREAM)?;
    let evidence = Evidence::read(&base)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (owner, original) in fixtures.iter().enumerate() {
        let records = rows.iter().enumerate().filter(|(_, row)| {
            matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        }).collect::<Vec<_>>();
        let (ordinal, event) = original_row(&records, EVENT, &[1, 0])?;
        check_event(event, original);
        let raw = event
            .source
            .raw
            .as_ref()
            .ok_or("actual retained soundbank")?;
        assert_eq!(
            fs::read(base.join(&raw.path))?,
            original.bytes,
            "exact raw source and ignored Event suffix"
        );
        evidence.exact(
            ordinal,
            owner,
            "/data/payload/action_count",
            "eventActionCount",
            &original.count,
        )?;
        check_actions(&records, original, &evidence, owner)?;
        let (_, tail) = original_row(&records, TAIL, &[1, 0])?;
        assert_eq!(
            tail.data.pointer("/payload/trailing_range"),
            Some(&range(&original.trailing)),
            "exact original ignored suffix"
        );
        for (_, row) in records {
            assert!(
                validator.is_valid(&serde_json::to_value(row)?),
                "Event scope record validates"
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_event_count_and_unknown_version_keep_raw_objects_without_action_rows() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixtures = [fixture::malformed()?, fixture::unsupported()?];
    check_legacy_layouts(&fixtures)?;
    let archive = archive(folder.path(), &fixtures)?;
    let output = folder.path().join("out");
    let normal = p4kx_media::export(&archive, &output, &options())
        .err()
        .ok_or("an impossible Event count is projection loss")?;
    assert!(
        matches!(
            layout_health::leaf(&normal),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "typed normal failure"
    );
    let diagnostics = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &diagnostics)?
    else {
        return Err("diagnostic Event inventory".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation base")?;
    let records = support::records(base, &index, AUDIO_STREAM)?;
    check_outcomes(&records, base, &fixtures)
}

fn check_outcomes(records: &[MediaRecord], base: &Path, fixtures: &[Fixture; 2]) -> TestResult {
    assert!(
        records
            .iter()
            .all(|row| !matches!(row.subresource.namespace.as_str(), ACTION | TAIL)),
        "no semantic action or tail is invented"
    );
    let events = records
        .iter()
        .filter(|row| row.subresource.namespace == EVENT)
        .collect::<Vec<_>>();
    let [malformed, unsupported] = events.as_slice() else {
        return Err("two physical Event outcomes".into());
    };
    assert_eq!(
        malformed.status,
        Status::Partial,
        "impossible original count is diagnostic loss"
    );
    assert_eq!(
        malformed.data.pointer("/payload/layout"),
        Some(&json!("malformed")),
        "distinct known-layout failure"
    );
    assert_eq!(
        malformed.data.pointer("/payload/action_count"),
        Some(&json!(3)),
        "readable original count survives"
    );
    assert_eq!(
        unsupported.status,
        Status::Complete,
        "unsupported layout retains complete physical inventory"
    );
    assert_eq!(
        unsupported.data.pointer("/payload/layout"),
        Some(&json!("unsupported")),
        "unsupported version is explicit"
    );
    assert_eq!(
        unsupported.data.pointer("/payload/action_count"),
        Some(&Value::Null),
        "unknown layout yields no decoded count"
    );
    for (row, original) in [*malformed, *unsupported].into_iter().zip(fixtures) {
        let raw = row
            .source
            .raw
            .as_ref()
            .ok_or("retained original unresolved event")?;
        assert_eq!(
            fs::read(base.join(&raw.path))?,
            original.bytes,
            "both physical raw bodies survive"
        );
    }
    Ok(())
}

fn check_event(event: &MediaRecord, original: &Fixture) {
    assert_eq!(
        event.status,
        Status::Complete,
        "known Event layout is readable"
    );
    assert_eq!(
        event.data.get("record_type"),
        Some(&json!("bank-hirc-event")),
        "records typed Event"
    );
    assert_eq!(
        event.data.pointer("/payload/layout"),
        Some(&json!("decoded")),
        "legacy decoded Event status"
    );
    assert_eq!(
        event.data.pointer("/payload/action_count"),
        Some(&json!(original.actions.len())),
        "authored count"
    );
    assert_eq!(
        event.data.pointer("/payload/count_encoding"),
        Some(&json!(original.encoding())),
        "version-selected encoding"
    );
    assert_eq!(
        event.data.pointer("/payload/count_range"),
        Some(&range(&original.count)),
        "original count width and range"
    );
}

fn check_actions(
    records: &[(usize, &MediaRecord)],
    original: &Fixture,
    evidence: &Evidence,
    owner: usize,
) -> TestResult {
    let actions = records
        .iter()
        .filter(|(_, row)| row.subresource.namespace == ACTION)
        .collect::<Vec<_>>();
    assert_eq!(
        actions.len(),
        original.actions.len(),
        "stream includes every original action"
    );
    for (index, ((position, row), id)) in actions.into_iter().zip(&original.actions).enumerate() {
        let extent = original.action_range(index);
        assert_eq!(
            row.subresource.indices,
            [1, 0, u64::try_from(index)?],
            "original action ordinal"
        );
        assert_eq!(
            row.data.pointer("/payload/action_id"),
            Some(&json!(id)),
            "original action ID and duplicates"
        );
        assert_eq!(
            row.data.pointer("/payload/id_range"),
            Some(&range(&extent)),
            "exact original action word"
        );
        evidence.exact(
            *position,
            owner,
            "/data/payload/action_id",
            "eventActionId",
            &extent,
        )?;
    }
    Ok(())
}

fn check_legacy_layouts(fixtures: &[Fixture; 2]) -> TestResult {
    for (original, expected) in fixtures
        .iter()
        .zip([HircLayout::Malformed, HircLayout::Unsupported])
    {
        let bank = Soundbank::parse(original.bytes.as_slice())?;
        let event = bank.objects().first().ok_or("physical legacy event")?;
        assert_eq!(event.layout(), expected, "legacy Event layout oracle");
    }
    Ok(())
}

fn original_row<'record>(
    rows: &[(usize, &'record MediaRecord)],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'record MediaRecord)> {
    rows.iter()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices.as_slice() == indices
        })
        .copied()
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn archive(directory: &Path, fixtures: &[Fixture; 2]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SAME_NAME, fixtures[0].bytes.as_slice()))
        .entry(TestEntry::zstd(SAME_NAME, fixtures[1].bytes.as_slice()).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}
