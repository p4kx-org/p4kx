// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Source};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestContainer, TestEntry, TestHircObject, test_key};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::TestResult;

#[path = "audio_foreign_bank_provider_checks.rs"]
mod checks;

use self::checks::{
    check_filtered, check_nested_queries, check_origins, check_receipts, check_root_queries,
};

const OWNER: &str = "owner.bnk";
const DUPLICATE: &str = "dup.bnk";
const OTHER: &str = "other.bnk";
const NESTED: &str = "child.socpak";
const ACTION: &str = "audio-bank-hirc-action-target-resolution";
const SEARCH: &str = "audio-bank-foreign-target-resolution";
const CANDIDATE: &str = "audio-bank-foreign-target-candidate";
const BLOCKER: &str = "audio-bank-foreign-target-blocker";
const PLAY: u16 = 0x0403;
const PLAY_EVENT: u16 = 0x2103;
const TARGET: u32 = 900;
const ABSENT: u32 = 999;
const TARGET_BANK: u32 = 91;
const OTHER_BANK: u32 = 92;
const ABSENT_BANK: u32 = 95;

#[test]
fn foreign_queries_join_exact_duplicate_banks_and_objects_within_each_owning_directory()
-> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let nested = TestContainer::new()
        .entry(TestEntry::stored(OWNER, owner().bytes()))
        .entry(TestEntry::stored(DUPLICATE, event_bank(ABSENT_BANK)))
        .build();
    let first = TestBank::new(TARGET_BANK)
        .object(TestHircObject::event(TARGET, &[]))
        .object(TestHircObject::sound(TARGET, 2, 2, 0))
        .object(TestHircObject::event(TARGET, &[]))
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(OWNER, owner().bytes()).encrypted())
        .entry(TestEntry::stored(DUPLICATE, first))
        .entry(TestEntry::stored(DUPLICATE, event_bank(TARGET_BANK)))
        .entry(TestEntry::stored(OTHER, event_bank(OTHER_BANK)))
        .entry(TestEntry::stored(NESTED, nested))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = directory.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    check_root_queries(&rows)?;
    check_nested_queries(&rows)?;
    let base = output.join(&index.artifact_base);
    check_receipts(&base)?;
    check_origins(&base, &rows)?;
    validate(&rows)
}

#[test]
fn filtered_banks_report_both_original_access_flags_without_reading_payloads() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(OWNER, owner().bytes()))
        .entry(TestEntry::stored(DUPLICATE, event_bank(TARGET_BANK)).encrypted())
        .entry(TestEntry::stored(OTHER, event_bank(OTHER_BANK)))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = directory.path().join("out");
    let options = ExportOptions {
        include: Some(Filter::parse(OWNER)?),
        ..options()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    check_filtered(&output.join(&index.artifact_base), &rows)?;
    validate(&rows)
}

#[test]
fn incomplete_hirc_preserves_prefix_and_nonmatching_bank_ids_still_exclude_it() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut bytes = event_bank(OTHER_BANK);
    let count = bytes
        .windows(4)
        .position(|word| word == b"HIRC")
        .ok_or("HIRC")?
        + 8;
    bytes
        .get_mut(count..count + 4)
        .ok_or("HIRC object count")?
        .copy_from_slice(&2_u32.to_le_bytes());
    ArchiveBuilder::new()
        .entry(TestEntry::stored(OWNER, owner().bytes()))
        .entry(TestEntry::stored(OTHER, bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = directory.path().join("out");
    let rows = diagnostic_rows(&archive, &output)?;
    let (_, play) = row_at(&rows, SEARCH, &[0], root)?;
    assert_eq!(
        play.data.pointer("/payload/resolution"),
        Some(&json!("missing"))
    );
    let (_, event) = row_at(&rows, SEARCH, &[2], root)?;
    assert_eq!(
        event.data.pointer("/payload/resolution"),
        Some(&json!("unavailable"))
    );
    assert_eq!(candidates(&rows, 2, root)?, [(1, 0, 4)]);
    let (_, blocker) = row_at(&rows, BLOCKER, &[2, 0], root)?;
    assert_eq!(
        blocker.data.pointer("/payload/reason"),
        Some(&json!("incomplete-hirc"))
    );
    validate(&rows)
}

#[test]
fn duplicate_bkhd_cannot_prove_a_foreign_bank_id() -> TestResult {
    const SECTION_HEADER_BYTES: usize = 8;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let other = TestBank::new(OTHER_BANK).bytes();
    let second_header = other.get(SECTION_HEADER_BYTES..).ok_or("BKHD body")?;
    let duplicate = TestBank::new(TARGET_BANK)
        .object(TestHircObject::event(TARGET, &[]))
        .section(*b"BKHD", second_header)
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(OWNER, owner().bytes()))
        .entry(TestEntry::stored(DUPLICATE, duplicate))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = directory.path().join("out");
    let rows = diagnostic_rows(&archive, &output)?;
    for query in [0, 1, 2, 3] {
        let (_, summary) = row_at(&rows, SEARCH, &[query], root)?;
        assert_eq!(
            summary.data.pointer("/payload/resolution"),
            Some(&json!("unavailable"))
        );
        assert_eq!(
            candidates(&rows, query, root)?,
            [],
            "duplicate BKHD prevents a proved target key"
        );
        let (_, blocker) = row_at(&rows, BLOCKER, &[query, 0], root)?;
        assert_eq!(
            blocker.data.pointer("/payload/reason"),
            Some(&json!("ambiguous-bank-header"))
        );
    }
    validate(&rows)
}

fn diagnostic_rows(archive: &Archive, output: &Path) -> TestResult<Vec<MediaRecord>> {
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(archive, output, &options)?
    else {
        return Err("malformed foreign BNK must leave an incomplete export".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic directory")?;
    support::records(base, &index, "audio.jsonl")
}

fn owner() -> TestBank {
    TestBank::new(7)
        .object(TestHircObject::action_in_bank(1, PLAY, TARGET, TARGET_BANK))
        .object(TestHircObject::action_in_bank(2, PLAY, TARGET, TARGET_BANK))
        .object(TestHircObject::action_in_bank(3, PLAY, TARGET, ABSENT_BANK))
        .object(TestHircObject::action(4, PLAY_EVENT, TARGET))
        .object(TestHircObject::action(5, PLAY_EVENT, ABSENT))
}

fn event_bank(id: u32) -> Vec<u8> {
    TestBank::new(id)
        .object(TestHircObject::event(TARGET, &[]))
        .bytes()
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}

const fn root(source: &Source) -> bool {
    matches!(source.identity, Identity::Root)
}

fn row_at<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    indices: &[u64],
    owner: impl Fn(&Source) -> bool,
) -> TestResult<(usize, &'rows MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace
                && row.subresource.indices == indices
                && owner(&row.source)
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn candidates(
    rows: &[MediaRecord],
    query: u64,
    owner: impl Fn(&Source) -> bool,
) -> TestResult<Vec<(u64, u64, u64)>> {
    let mut result = Vec::new();
    for row in rows.iter().filter(|row| {
        row.subresource.namespace == CANDIDATE
            && row.subresource.indices.first() == Some(&query)
            && owner(&row.source)
    }) {
        let payload = row.data.get("payload").ok_or("candidate payload")?;
        let entry = payload["target_entry_index"]
            .as_str()
            .ok_or("entry")?
            .parse()?;
        let object = payload["target_object_ordinal"]
            .as_str()
            .ok_or("object")?
            .parse()?;
        let kind = payload["target_type_code"].as_u64().ok_or("type code")?;
        assert_eq!(
            payload["target_bkhd_section_ordinal"],
            json!("0"),
            "candidate cites the original BKHD"
        );
        assert_eq!(
            payload["target_hirc_section_ordinal"],
            json!("1"),
            "candidate cites the original HIRC"
        );
        assert_eq!(
            row.subresource.indices,
            [query, result.len() as u64],
            "candidate ordinal follows physical order"
        );
        result.push((entry, object, kind));
    }
    Ok(result)
}

fn validate(rows: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows.iter().filter(|row| {
        matches!(
            row.subresource.namespace.as_str(),
            ACTION | SEARCH | CANDIDATE | BLOCKER
        )
    }) {
        let mut row_value = serde_json::to_value(row)?;
        assert!(schema.is_valid(&row_value), "invalid row: {row_value}");
        if row.subresource.namespace == ACTION
            && row.data.pointer("/payload/foreign/status") == Some(&json!("directory-search"))
        {
            let foreign = row_value
                .pointer_mut("/data/payload/foreign")
                .and_then(Value::as_object_mut)
                .ok_or("foreign Action reference")?;
            foreign.remove("query_index");
            assert!(
                !schema.is_valid(&row_value),
                "directory search requires an exact query key"
            );
        }
    }
    Ok(())
}
