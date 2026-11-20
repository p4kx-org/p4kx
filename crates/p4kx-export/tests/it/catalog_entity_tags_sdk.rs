// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::tags::{
    EntityTagSnapshot, with_entity_tags, with_entity_tags_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordReference, CatalogRecordSelection,
    DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    CancelToken, DataForgeEntry, FieldOrigin, InputScope, SourceUseRole, SourceValueState,
    SourcedValue,
};
use p4kx_testkit::TestValue;
use serde_json::{Value, json};

use crate::catalog_tags_fixture as fixture;
use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin, TestResult};

const TABLE: &str = "/declarations/entityTags";
const ITEM_RECORD: u32 = 5;
const ALIAS_RECORD: u32 = 7;
const TAG_RECORD: u32 = 0;

#[test]
fn entity_tag_sdk_matches_catalog_rows_and_original_occurrence_evidence() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::entities(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = archive_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    with_entity_tags_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            assert_eq!(
                Some(&json!({"availability": snapshot.availability, "rows": snapshot.rows})),
                catalog.pointer(TABLE),
                "SDK and staged Catalog preserve the same canonical rows and references"
            );
            let item = snapshot
                .rows
                .iter()
                .find(|row| row.source().record_index().value() == Some(&ITEM_RECORD))
                .ok_or("declared physical Entity")?;
            verify_occurrences(
                &snapshot,
                &catalog,
                item.tags().value().ok_or("authored tag array")?,
            )
        },
    )?
}

fn verify_occurrences(
    snapshot: &EntityTagSnapshot<'_>,
    catalog: &Value,
    tags: &[SourcedValue<CatalogRecordReference>],
) -> TestResult {
    let [first, repeated, last] = tags else {
        return Err("three physical tag array elements".into());
    };
    for (cell, index) in [(first, 1), (repeated, 1), (last, 2)] {
        let reference = cell.value().ok_or("original tag reference")?;
        assert_eq!(
            reference.target_record_index().value(),
            Some(&index),
            "each authored occurrence preserves the resolved physical Tag header"
        );
        let origin = cell.origin();
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "each original array occurrence came from encrypted DataForge"
        );
        assert!(
            origin.source_set_complete(),
            "the occurrence has complete source evidence"
        );
        verify_occurrence_sources(snapshot, catalog, origin)?;
    }
    assert_ne!(
        first.origin().source_uses_refs(),
        repeated.origin().source_uses_refs(),
        "repeated GUIDs retain distinct locations in the original array"
    );
    Ok(())
}

fn verify_occurrence_sources(
    snapshot: &EntityTagSnapshot<'_>,
    catalog: &Value,
    origin: &FieldOrigin,
) -> TestResult {
    let producer = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("producer source-use ledger")?;
    let mut payload = false;
    for reference in origin.source_uses_refs() {
        let used = snapshot
            .source_inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("SDK occurrence source use")?;
        assert!(
            producer.contains(&serde_json::to_value(used)?),
            "the producer retains the same physical source, role and occurrence coordinate"
        );
        payload |= used.scope() == InputScope::Payload && used.role() == SourceUseRole::Payload;
    }
    assert!(
        payload,
        "each selected occurrence is anchored in the Entity payload"
    );
    Ok(())
}

#[test]
fn entity_tag_sdk_keeps_aliases_and_selects_the_physical_owner() -> TestResult {
    let database = fixture::entities().record(fixture::entity(
        fixture::ITEM,
        "Alias",
        "scitem/ships/powerplant/alias",
        Some(TestValue::Array(vec![
            TestValue::Null,
            TestValue::Reference(fixture::ROOT),
        ])),
    ));
    let (_directory, archive) = archive_fixture::open(&database, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge
        .record(ITEM_RECORD)
        .ok_or("original Entity owner")?
        .guid();
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (selector, expected_indices) in [
        (
            CatalogRecordSelection::Guid(guid),
            [ITEM_RECORD, ALIAS_RECORD].as_slice(),
        ),
        (
            CatalogRecordSelection::RecordIndex(ALIAS_RECORD),
            [ALIAS_RECORD].as_slice(),
        ),
        (
            CatalogRecordSelection::RecordIndex(TAG_RECORD),
            [].as_slice(),
        ),
    ] {
        parsed.with_entity_tags(selector, None, |snapshot| {
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "family availability follows the source declaration"
            );
            assert_eq!(
                selected_positions(&snapshot)?,
                expected_indices,
                "only admitted physical Entity Records"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        })??;
    }
    parsed.with_entity_tags(
        CatalogRecordSelection::RecordIndex(ALIAS_RECORD),
        None,
        |snapshot| verify_alias_tags(&snapshot),
    )?
}

fn verify_alias_tags(snapshot: &EntityTagSnapshot<'_>) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one exact physical alias".into());
    };
    let Some([empty, root]) = row.tags().value().map(Vec::as_slice) else {
        return Err("one null and one authored tag array slot".into());
    };
    assert_eq!(
        empty.state(),
        SourceValueState::Null,
        "the source null keeps its ordinal"
    );
    assert_eq!(
        root.value()
            .and_then(|tag| tag.target_record_index().value()),
        Some(&TAG_RECORD),
        "the physical alias retains its own array"
    );
    Ok(())
}

fn selected_positions(snapshot: &EntityTagSnapshot<'_>) -> TestResult<Vec<u32>> {
    snapshot
        .rows
        .iter()
        .map(|row| {
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or_else(|| "original Entity Record index".into())
        })
        .collect()
}

#[test]
fn entity_tag_sdk_cancellation_does_not_publish_a_snapshot() -> TestResult {
    let (_directory, archive) = archive_fixture::open(&fixture::entities(), DcbOrigin::Plain)?;
    let token = CancelToken::new();
    token.cancel();
    let result = with_entity_tags(&archive, CatalogRecordSelection::All, Some(&token), |_| ());
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "cancellation interrupts the query"
    );
    Ok(())
}
