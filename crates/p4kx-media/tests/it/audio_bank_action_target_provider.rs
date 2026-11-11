// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{Origins, Owner, TestResult, read_rows};

const BANK: &str = "owner.bnk";
const OTHER_BANK: &str = "foreign.bnk";
const SUMMARY: &str = "audio-bank-hirc-action-target-resolution";
const CANDIDATE: &str = "audio-bank-hirc-action-target-candidate";
const PLAY: u16 = 0x0403;
const PLAY_EVENT: u16 = 0x2103;
const TRIGGER: u16 = 0x1B03;
const FOREIGN: u32 = 91;
const MIXED: u32 = 42;
const SOUND_ONLY: u32 = 71;
const REMOTE: u32 = 900;

#[test]
fn action_targets_keep_local_candidates_and_distinguish_unread_foreign_selectors() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bank = TestBank::new(7)
        .object(TestHircObject::action_in_bank(1, PLAY, MIXED, FOREIGN))
        .object(TestHircObject::action(2, PLAY_EVENT, MIXED))
        .object(TestHircObject::action_in_bank(3, PLAY, REMOTE, FOREIGN))
        .object(TestHircObject::action(4, PLAY_EVENT, SOUND_ONLY))
        .object(TestHircObject::action(5, PLAY, REMOTE))
        .object(TestHircObject::action(6, TRIGGER, MIXED))
        .object(TestHircObject::sound(MIXED, 2, 2, 0))
        .object(TestHircObject::event(MIXED, &[]))
        .object(TestHircObject::event(MIXED, &[]))
        .object(TestHircObject::sound(SOUND_ONLY, 2, 2, 0))
        .bytes();
    let archive = archive(directory.path(), bank)?;
    let output = directory.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options(false)?)?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    check_complete_summaries(&rows)?;
    for (candidate, target, kind, event_match) in
        [(0, 6, 2, false), (1, 7, 4, true), (2, 8, 4, true)]
    {
        check_candidate(&rows, [1, 0, candidate], target, kind, true)?;
        check_candidate(&rows, [1, 1, candidate], target, kind, event_match)?;
    }
    check_candidate(&rows, [1, 3, 0], 9, 2, false)?;
    assert!(
        rows.iter().all(|row| row.subresource.namespace != SUMMARY
            || row.subresource.indices.as_slice() != [1, 5]),
        "Trigger does not use a Play or PlayEvent target route"
    );
    check_encrypted_candidate_origins(&output.join(&index.artifact_base), &rows)?;
    validate(&rows)
}

fn check_complete_summaries(rows: &[MediaRecord]) -> TestResult {
    let bank = json!({"type":"bank","bank_id":FOREIGN});
    let event = json!({"type":"event"});
    let expected = [
        (
            0,
            MIXED,
            "ambiguous",
            3,
            3,
            json!({"status":"local-match","target":bank}),
        ),
        (
            1,
            MIXED,
            "ambiguous",
            3,
            2,
            json!({"status":"local-match","target":event}),
        ),
        (
            2,
            REMOTE,
            "missing",
            0,
            0,
            unavailable(&bank, "target-not-read"),
        ),
        (
            3,
            SOUND_ONLY,
            "missing",
            1,
            0,
            unavailable(&event, "target-not-read"),
        ),
        (4, REMOTE, "missing", 0, 0, json!({"status":"local-only"})),
    ];
    for (action, id, local, observed, matched, foreign) in expected {
        check_summary(rows, action, id, (local, observed, matched, true), &foreign)?;
    }
    Ok(())
}

#[test]
fn incomplete_hirc_retains_observed_matches_and_marks_unread_local_precedence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mut bank = TestBank::new(7)
        .object(TestHircObject::action(1, PLAY_EVENT, MIXED))
        .object(TestHircObject::action_in_bank(2, PLAY, REMOTE, FOREIGN))
        .object(TestHircObject::event(MIXED, &[]))
        .bytes();
    let hirc = bank
        .windows(4)
        .position(|word| word == b"HIRC")
        .ok_or("fixture HIRC")?
        + 8;
    bank.get_mut(hirc..hirc + 4)
        .ok_or("original object count")?
        .copy_from_slice(&4_u32.to_le_bytes());
    let archive = archive(directory.path(), bank)?;
    let output = directory.path().join("out");
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options(true)?)?
    else {
        return Err("truncated HIRC must produce diagnostics".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic directory")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    check_summary(
        &rows,
        0,
        MIXED,
        ("unavailable", 1, 1, false),
        &json!({"status":"local-match","target":{"type":"event"}}),
    )?;
    check_summary(
        &rows,
        1,
        REMOTE,
        ("unavailable", 0, 0, false),
        &unavailable(
            &json!({"type":"bank","bank_id":FOREIGN}),
            "incomplete-local-inventory",
        ),
    )?;
    check_candidate(&rows, [1, 0, 0], 2, 4, true)?;
    check_incomplete_origin(&base.join(&index.artifact_base), &rows)?;
    validate(&rows)
}

fn archive(directory: &Path, bank: Vec<u8>) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    let other = TestBank::new(FOREIGN)
        .object(TestHircObject::event(REMOTE, &[]))
        .object(TestHircObject::event(SOUND_ONLY, &[]))
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BANK, bank).encrypted())
        .entry(TestEntry::stored(OTHER_BANK, other))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn options(diagnostic: bool) -> TestResult<ExportOptions> {
    let mut options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        include: Some(Filter::parse(BANK)?),
        diagnostic,
        ..ExportOptions::default()
    };
    options.limits.reference_candidates = 1;
    Ok(options)
}

fn unavailable(target: &Value, reason: &str) -> Value {
    json!({"status":"unavailable","target":target,"reason":reason})
}

fn check_summary(
    rows: &[MediaRecord],
    action: u64,
    target: u32,
    local: (&str, u64, u64, bool),
    foreign: &Value,
) -> TestResult {
    let (_, row) = row_at(rows, SUMMARY, &[1, action])?;
    let (resolution, observed, matching, complete) = local;
    assert_eq!(
        row.data,
        json!({
            "record_type":"bank-hirc-action-target-resolution",
            "payload":{
                "target_id":target,
                "local":{
                    "resolution":resolution,"observed_candidates":observed.to_string(),
                    "matching_candidates":matching.to_string(),"candidates_complete":complete
                },
                "foreign":foreign
            }
        }),
        "the authored target reports local inventory and foreign availability independently"
    );
    Ok(())
}

fn check_candidate(
    rows: &[MediaRecord],
    indices: [u64; 3],
    target: u64,
    kind: u8,
    matches: bool,
) -> TestResult {
    let (_, row) = row_at(rows, CANDIDATE, &indices)?;
    assert_eq!(
        row.data,
        json!({
            "record_type":"bank-hirc-action-target-candidate",
            "payload":{
                "target_section_ordinal":"1","target_object_ordinal":target.to_string(),
                "target_type_code":kind,"matches_target_kind":matches
            }
        }),
        "the physical candidate keeps its type and occurrence"
    );
    row_at(rows, "audio-bank-object", &[1, target])?;
    Ok(())
}

fn check_encrypted_candidate_origins(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let (ordinal, _) = row_at(rows, CANDIDATE, &[1, 1, 0])?;
    let origins = Origins::read(base)?;
    let receipt = origins.field(
        "records",
        ordinal,
        "/data/payload/matches_target_kind",
        true,
    )?;
    let uses = origins.require_owner(
        receipt,
        Owner {
            path: BANK,
            entry: 0,
            parent: None,
            stored: "encrypted",
            metadata: false,
            payload: true,
        },
    )?;
    for field in ["actionType", "targetId", "objectId", "typeCode"] {
        assert!(
            uses.iter()
                .any(|used| used.pointer("/location/field") == Some(&json!(field))),
            "the encrypted candidate records its original {field}"
        );
    }
    let foreign = row_at(rows, SUMMARY, &[1, 2])?.0;
    let receipt = origins.field(
        "records",
        foreign,
        "/data/payload/foreign/target/bank_id",
        true,
    )?;
    let uses = origins.require_owner(
        receipt,
        Owner {
            path: BANK,
            entry: 0,
            parent: None,
            stored: "encrypted",
            metadata: false,
            payload: true,
        },
    )?;
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/field") == Some(&json!("bankId"))
                && used.get("role") == Some(&json!("selector"))
        }),
        "the foreign selector cites the original bank ID"
    );
    Ok(())
}

fn check_incomplete_origin(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let ordinal = row_at(rows, SUMMARY, &[1, 1])?.0.to_string();
    let origins: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    let field = origins
        .iter()
        .find(|origin| {
            origin.pointer("/scope/stream/kind") == Some(&json!("audio"))
                && origin.pointer("/scope/ordinal") == Some(&json!(ordinal))
                && origin.get("field") == Some(&json!("/data/payload/local/resolution"))
        })
        .ok_or("local resolution origin")?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(true)),
        "known Action and HIRC selectors originate in the encrypted bank"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&json!(false)),
        "unknown local candidates remain unknown in the classification receipt"
    );
    Ok(())
}

fn row_at<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'rows MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn validate(rows: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows
        .iter()
        .filter(|row| matches!(row.subresource.namespace.as_str(), SUMMARY | CANDIDATE))
    {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "actual Action target row validates"
        );
    }
    Ok(())
}
