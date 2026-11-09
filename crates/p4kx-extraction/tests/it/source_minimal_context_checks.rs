// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Containing-domain evidence and retained prefix rows.

use p4kx_extraction::{
    Archive, InputScope, SourceDetails, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, SourceValue,
};
use serde_json::Value;

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

pub(super) fn upgraded_unknown_source(sources: &SourceUses, sparse: &SourceScope) -> TestResult {
    support::assert_closed(&sources.freeze()?)?;
    let wire = serde_json::to_value(sources.freeze()?)?;
    let rows = wire
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("physical rows")?;
    let upgraded = rows
        .iter()
        .find(|row| {
            row.get("id")
                .is_some_and(|id| id == sparse.source_ref().as_str())
        })
        .ok_or("upgraded original row")?;
    assert_eq!(
        upgraded.get("kind").ok_or("upgraded original row")?,
        "archiveEntry",
        "full original details replace the actual sparse leaf"
    );
    assert_eq!(
        upgraded
            .get("storedEncryption")
            .ok_or("upgraded original row")?,
        "unencrypted",
        "a missing parent does not erase known leaf declarations"
    );
    assert_eq!(
        upgraded
            .get("path")
            .and_then(|path| path.get("containsEncryptedSource"))
            .ok_or("upgraded original row")?,
        &Value::Null,
        "unproved containing context remains unknown in full metadata facts"
    );
    Ok(())
}

pub(super) fn prefix_domains(
    sources: &SourceUses,
    outer_scope: &SourceScope,
    inner_scope: &SourceScope,
    leaf_scope: &SourceScope,
) -> TestResult {
    let cases = [
        (
            outer_scope,
            InputScope::ArchiveDirectoryMetadata,
            Some(false),
        ),
        (
            inner_scope,
            InputScope::ArchiveDirectoryMetadata,
            Some(false),
        ),
        (inner_scope, InputScope::Payload, Some(true)),
        (leaf_scope, InputScope::ArchiveDirectoryMetadata, Some(true)),
    ];
    for (scope, domain, marker) in cases {
        let value = sources.value(
            SourceValue::Present(()),
            &[support::used(&scope.for_scope(domain))?],
            true,
        );
        assert_eq!(
            value.origin().contains_encrypted_source(),
            marker,
            "retained original prefix governs {domain:?}"
        );
    }
    Ok(())
}

pub(super) fn prefix_rows(sources: &SourceUses) -> TestResult {
    support::assert_closed(&sources.freeze()?)?;
    let wire = serde_json::to_value(sources.freeze()?)?;
    let rows = wire
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("physical rows")?;
    assert_eq!(
        rows.len(),
        3,
        "all actual containing/leaf sources survive optional detail omission"
    );
    assert!(
        rows.iter().all(|row| row
            .get("kind")
            .is_some_and(|kind| kind == "archiveEntryProof")),
        "no variable directory details are copied by the minimal path"
    );
    assert_eq!(
        rows.get(2)
            .and_then(|row| row.get("ancestors"))
            .and_then(Value::as_array)
            .ok_or("leaf ancestry")?
            .len(),
        2,
        "original containing-source closure survives capture limits"
    );
    Ok(())
}

pub(super) fn encrypted_second_source(
    sources: &SourceUses,
    archive: &Archive,
) -> TestResult<SourceScope> {
    let encrypted_entry = archive
        .entries()
        .get(1)
        .ok_or("encrypted second original Entry")?;
    let encrypted = sources.observe_with_details(
        SourceInput::ArchiveView(
            archive
                .observed_entry(encrypted_entry)
                .ok_or("encrypted original view")?,
        ),
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    Ok(encrypted)
}

pub(super) fn incomplete_unknown_upgrade(
    sources: &SourceUses,
    full: &SourceScope,
    one: &Archive,
) -> TestResult {
    let marker = sources.value(SourceValue::Present(()), &[support::used(full)?], true);
    assert_eq!(
        marker.origin().contains_encrypted_source(),
        None,
        "detail upgrade does not repair unproved ancestry"
    );
    let limited = support::source(sources, one, SourceDetails::CaptureLimited)?;
    assert_eq!(
        limited.source_ref(),
        full.source_ref(),
        "later limited observations retain upgraded unproved-view identity"
    );
    Ok(())
}

pub(super) fn plain_selector_uses(plain: &SourceScope, count: u64) -> TestResult<Vec<SourceUse>> {
    let mut uses = Vec::new();
    for ordinal in 0..count {
        uses.push(plain.at(
            SourceLocation::Subresource {
                namespace: "synthetic".to_owned(),
                indices: vec![ordinal],
                field: "name".to_owned(),
            },
            SourceUseRole::Selector,
        )?);
    }
    Ok(uses)
}
