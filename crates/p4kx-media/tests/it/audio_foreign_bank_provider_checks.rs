// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_media::{Identity, MediaRecord, Source};
use serde_json::{Value, json};

use crate::support::audio_provider::{Origins, Owner, TestResult, read_rows};

use super::{
    ACTION, BLOCKER, CANDIDATE, DUPLICATE, NESTED, OTHER, OWNER, SEARCH, TARGET, TARGET_BANK,
    candidates, root, row_at,
};

pub(super) fn check_root_queries(rows: &[MediaRecord]) -> TestResult {
    let owner = |source: &Source| {
        matches!(&source.identity, Identity::Entry { containers, entry }
            if containers.is_empty() && entry.entry_index == 0)
    };
    for (action, query) in [(0, 0), (1, 0), (2, 1), (3, 2), (4, 3)] {
        let (_, row) = row_at(rows, ACTION, &[1, action], owner)?;
        assert_eq!(
            row.data.pointer("/payload/foreign/query_index"),
            Some(&json!(query.to_string())),
            "Action {action} joins the deduplicated owner query"
        );
    }
    let selector = json!({"type":"bank","bank_id":TARGET_BANK});
    let (_, play) = row_at(rows, SEARCH, &[0], root)?;
    assert_eq!(
        play.data,
        json!({"record_type":"bank-foreign-target-resolution","payload":{
            "owner_entry_index":"0","target_id":TARGET,"selector":selector,
            "directory_complete":true,"candidates_complete":true,
            "observed_candidates":"4","matching_candidates":"4","blocked_banks":"0",
            "resolution":"ambiguous"
        }}),
        "both same-name target banks and every duplicate HIRC object count"
    );
    let expected = [(1, 0, 4), (1, 1, 2), (1, 2, 4), (2, 0, 4), (3, 0, 4)];
    assert_eq!(
        candidates(rows, 0, root)?,
        expected[..4],
        "Play scans the named BKHD IDs"
    );
    assert_eq!(
        candidates(rows, 2, root)?,
        expected,
        "PlayEvent observes every scoped holder"
    );
    let (_, wrong_kind) = row_at(rows, CANDIDATE, &[2, 1], root)?;
    assert_eq!(
        wrong_kind.data.pointer("/payload/matches_target_kind"),
        Some(&json!(false)),
        "a same-ID Sound is visible but cannot satisfy PlayEvent"
    );
    for (query, expected) in [(1, "missing"), (2, "ambiguous"), (3, "missing")] {
        let (_, row) = row_at(rows, SEARCH, &[query], root)?;
        assert_eq!(
            row.data.pointer("/payload/resolution"),
            Some(&json!(expected)),
            "complete root inventory decides query {query}"
        );
    }
    Ok(())
}

pub(super) fn check_nested_queries(rows: &[MediaRecord]) -> TestResult {
    let child = |source: &Source| {
        matches!(&source.identity, Identity::Entry { containers, entry }
            if containers.is_empty() && entry.entry_index == 4 && entry.path == NESTED)
    };
    let (_, query) = row_at(rows, SEARCH, &[1], child)?;
    assert_eq!(
        query.data.pointer("/payload/resolution"),
        Some(&json!("resolved")),
        "the root's missing Play target exists inside this nested directory"
    );
    assert_eq!(
        candidates(rows, 1, child)?,
        [(1, 0, 4)],
        "nested candidate uses the child Entry index"
    );
    assert_eq!(
        candidates(rows, 0, child)?,
        [],
        "the nested query cannot inherit root targets"
    );
    Ok(())
}

pub(super) fn check_receipts(base: &Path) -> TestResult {
    let sources: Vec<Source> = read_rows(base.join("sources.jsonl"))?;
    let banks: Vec<_> = sources
        .iter()
        .filter_map(|source| match &source.identity {
            Identity::Entry { containers, entry }
                if [OWNER, DUPLICATE, OTHER].contains(&entry.path.as_str()) =>
            {
                Some((containers.len(), entry.entry_index))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        banks,
        [(0, 0), (0, 1), (0, 2), (0, 3), (1, 0), (1, 1)],
        "directory-end replay preserves original raw receipt order and cardinality"
    );
    Ok(())
}

pub(super) fn check_origins(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let origins = Origins::read(base)?;
    let (index, _) = row_at(rows, SEARCH, &[0], root)?;
    let receipts: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    let directory = receipts
        .iter()
        .find(|row| {
            row.pointer("/scope/stream/kind") == Some(&json!("audio"))
                && row.pointer("/scope/ordinal") == Some(&json!(index.to_string()))
                && row.get("field") == Some(&json!("/data/payload/directory_complete"))
        })
        .ok_or("root Archive completeness receipt")?;
    assert_eq!(
        directory["sourceSetComplete"],
        json!(true),
        "root directory evidence is complete"
    );
    assert_eq!(
        directory["containsEncryptedSource"],
        Value::Null,
        "the root Archive completeness field has no leaf encryption state"
    );
    origins.field("records", index, "/data/payload/owner_entry_index", false)?;
    origins.field("records", index, "/data/payload/target_id", true)?;
    let (index, _) = row_at(rows, CANDIDATE, &[2, 3], root)?;
    origins.field("records", index, "/data/payload/matches_target_kind", true)?;
    let target = origins.field("records", index, "/data/payload/target_type_code", false)?;
    origins.require_owner(
        target,
        Owner {
            path: DUPLICATE,
            entry: 2,
            parent: None,
            stored: "unencrypted",
            metadata: false,
            payload: false,
        },
    )?;
    Ok(())
}

pub(super) fn check_filtered(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let (_, query) = row_at(rows, SEARCH, &[0], root)?;
    let summary = query.data.get("payload").ok_or("query payload")?;
    assert_eq!(
        summary["directory_complete"],
        json!(true),
        "the directory itself was read"
    );
    assert_eq!(
        summary["candidates_complete"],
        json!(false),
        "filtering blocks complete candidate coverage"
    );
    assert_eq!(
        summary["resolution"],
        json!("unavailable"),
        "filtering cannot prove absence"
    );
    assert_eq!(
        summary["blocked_banks"],
        json!("2"),
        "both filtered banks can affect Play"
    );
    assert_eq!(
        candidates(rows, 0, root)?,
        [],
        "no filtered payload can yield a candidate"
    );
    let origins = Origins::read(base)?;
    for (ordinal, reason, encrypted) in [(0, "key-required", true), (1, "filtered", false)] {
        let (index, row) = row_at(rows, BLOCKER, &[0, ordinal], root)?;
        assert_eq!(
            row.data.get("payload"),
            Some(&json!({
                "target_entry_index":(ordinal + 1).to_string(),
                "reason":reason,"filtered":true,"key_required":encrypted
            })),
            "blocked Entry {ordinal} preserves observed directory access flags"
        );
        origins.field("records", index, "/data/payload/key_required", false)?;
    }
    let sources: Vec<Source> = read_rows(base.join("sources.jsonl"))?;
    assert_eq!(
        sources.len(),
        1,
        "directory completion must not retain filtered banks"
    );
    Ok(())
}
