// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Minimal physical proofs and honest omitted coordinates in the original shared owner.

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_archive::Entry;
use p4kx_extraction::{
    Archive, InputScope, SourceDetails, SourceInput, SourceInventory, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses, SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};
use tempfile::TempDir;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) fn open(builder: &ArchiveBuilder) -> TestResult<(TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    builder.write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

pub(super) fn first(archive: &Archive) -> TestResult<&Entry> {
    archive
        .entries()
        .first()
        .ok_or_else(|| "actual Entry missing".into())
}

pub(super) fn source(
    sources: &SourceUses,
    archive: &Archive,
    details: SourceDetails,
) -> TestResult<SourceScope> {
    let view = archive
        .observed_entry(first(archive)?)
        .ok_or("actual source view")?;
    Ok(sources.observe_with_details(
        SourceInput::ArchiveView(view),
        InputScope::Payload,
        details,
    )?)
}

pub(super) fn used(scope: &SourceScope) -> TestResult<SourceUse> {
    Ok(scope.at(
        SourceLocation::Metadata {
            field: "fact".to_owned(),
        },
        SourceUseRole::Payload,
    )?)
}

#[test]
fn default_full_snapshot_and_borrowed_view_keep_identical_inventory_and_value_bytes() -> TestResult
{
    let (_directory, archive) =
        open(&ArchiveBuilder::new().entry(TestEntry::stored("source.bin", b"same").encrypted()))?;
    let origin = archive
        .entry_origin(first(&archive)?)
        .ok_or("full original source")?;
    let original = SourceUses::new();
    let old = original.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let borrowed = SourceUses::new();
    let new = source(&borrowed, &archive, SourceDetails::Full)?;
    let old_value = original.value(SourceValue::Present(7_u32), &[used(&old)?], true);
    let new_value = borrowed.value(SourceValue::Present(7_u32), &[used(&new)?], true);
    assert_eq!(
        serde_json::to_vec(&old_value)?,
        serde_json::to_vec(&new_value)?,
        "the borrowed Full API leaves value and use identities byte-identical"
    );
    assert_eq!(
        serde_json::to_vec(&original.freeze()?)?,
        serde_json::to_vec(&borrowed.freeze()?)?,
        "skipped original runtime bindings change no normal full inventory bytes"
    );
    let wire = serde_json::to_value(original.freeze()?)?;
    assert_eq!(
        wire["sources"][0]["kind"], "archiveEntry",
        "default retention remains the complete original variant"
    );
    assert!(
        wire["sources"][0].get("details").is_none(),
        "normal full rows gain no detail-control field"
    );
    assert!(
        wire["sources"][0].get("binding").is_none(),
        "runtime ownership is not serialized"
    );
    Ok(())
}

#[test]
fn capture_limited_physical_source_keeps_actual_encryption_scope_and_decimal_ordinal() -> TestResult
{
    let builder =
        ArchiveBuilder::new().entry(TestEntry::stored("not-copied.bin", b"source").encrypted());
    let (_directory, archive) = open(&builder)?;
    let (_other_directory, other) = open(&builder)?;
    let sources = SourceUses::new();
    let scope = source(&sources, &archive, SourceDetails::CaptureLimited)?;
    let view = archive
        .observed_entry(first(&archive)?)
        .ok_or("original view")?;
    assert!(
        scope.matches_entry(&view)?,
        "minimal proof retains original membership"
    );
    assert!(
        !scope.matches_entry(&other.observed_entry(first(&other)?).ok_or("other view")?)?,
        "same bytes and equal numeric index cannot replace the owner"
    );
    let payload = sources.value(SourceValue::Present(()), &[used(&scope)?], true);
    let metadata = sources.value(
        SourceValue::Present(()),
        &[used(
            &scope.for_scope(InputScope::ArchiveDirectoryMetadata),
        )?],
        true,
    );
    assert_eq!(
        payload.origin().contains_encrypted_source(),
        Some(true),
        "stored encrypted leaf remains a genuine payload contributor"
    );
    assert_eq!(
        metadata.origin().contains_encrypted_source(),
        Some(false),
        "leaf encryption does not taint its containing directory facts"
    );
    let wire = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        wire["sources"],
        json!([{"id":"a0.0","ancestors":[],"storedEncryption":"encrypted","kind":"archiveEntryProof","entryIndex":"0","details":"captureLimited"}]),
        "sparse proof carries real identity and a closed omission control without absent/unreadable details"
    );
    assert_eq!(
        wire["uses"].as_array().ok_or("actual use rows")?.len(),
        2,
        "proof-only retention registers no unrequested variable metadata uses"
    );
    checks::sparse_schema(&wire)?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "proof capture does not attempt payload decryption"
    );
    Ok(())
}

#[test]
fn sparse_to_full_upgrades_original_row_and_never_downgrades_or_rebinds_uses() -> TestResult {
    let (_directory, archive) =
        open(&ArchiveBuilder::new().entry(TestEntry::stored("full.bin", b"source").encrypted()))?;
    let sources = SourceUses::new();
    let sparse = source(&sources, &archive, SourceDetails::CaptureLimited)?;
    let original_use = used(&sparse)?;
    let before = sources.value(
        SourceValue::Present(3_u32),
        std::slice::from_ref(&original_use),
        true,
    );
    let full = source(&sources, &archive, SourceDetails::Full)?;
    assert_eq!(
        sparse.source_ref(),
        full.source_ref(),
        "detail retention upgrades the same original source identity"
    );
    let repeated = used(&full)?;
    assert_eq!(
        original_use.reference(),
        repeated.reference(),
        "existing actual use refs are stable across detail upgrade"
    );
    assert_eq!(
        serde_json::to_vec(&before)?,
        serde_json::to_vec(&sources.value(SourceValue::Present(3_u32), &[repeated], true))?,
        "field marker and original contribution closure are independent of retained detail level"
    );
    assert_closed(&sources.freeze()?)?;
    let full_wire = serde_json::to_vec(&sources.freeze()?)?;
    let again = source(&sources, &archive, SourceDetails::CaptureLimited)?;
    assert_eq!(
        full.source_ref(),
        again.source_ref(),
        "subsequent limited observation retains identity"
    );
    assert_eq!(
        full_wire,
        serde_json::to_vec(&sources.freeze()?)?,
        "a limited observation never discards full stored details"
    );
    let wire: Value = serde_json::from_slice(&full_wire)?;
    assert_eq!(
        wire["sources"].as_array().ok_or("physical rows")?.len(),
        1,
        "upgrade adds no duplicate source"
    );
    assert_eq!(
        wire["sources"][0]["path"]["value"], "full.bin",
        "full upgrade retains actual path only after admission"
    );
    assert_eq!(
        wire["sources"][0]["kind"], "archiveEntry",
        "full upgrade replaces proof-only variant"
    );
    Ok(())
}

#[test]
fn opaque_contributors_force_incompleteness_but_attempts_and_references_do_not_taint_literals()
-> TestResult {
    checks::opaque_contributors_force_incompleteness_but_attempts_and_references_do_not_taint_literals()
}

#[test]
fn opaque_conversion_preserves_original_owner_domain_role_and_known_encrypted_witness() -> TestResult
{
    checks::opaque_conversion_preserves_original_owner_domain_role_and_known_encrypted_witness()
}

pub(super) fn assert_closed(inventory: &SourceInventory) -> TestResult {
    let wire = serde_json::to_value(inventory)?;
    let rows = wire
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("physical source rows")?;
    let ids: BTreeSet<_> = rows
        .iter()
        .map(|row| {
            row.get("id")
                .and_then(Value::as_str)
                .ok_or("physical source ID")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        ids.len(),
        rows.len(),
        "one original physical row per identifier"
    );
    for row in rows {
        for ancestor in row
            .get("ancestors")
            .and_then(Value::as_array)
            .ok_or("source ancestry")?
        {
            assert!(
                ids.contains(ancestor.as_str().ok_or("ancestor ID")?),
                "every original ancestor ref resolves within the same frozen inventory"
            );
        }
    }
    let mut uses = BTreeSet::new();
    for used in inventory.uses() {
        assert!(
            ids.contains(used.source().as_str()),
            "every original use has a retained physical source"
        );
        assert!(uses.insert(used.id().as_str()), "original uses are unique");
    }
    Ok(())
}

#[path = "source_minimal_proof_checks.rs"]
mod checks;
