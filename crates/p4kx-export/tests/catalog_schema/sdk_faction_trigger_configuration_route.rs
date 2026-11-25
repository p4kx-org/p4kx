// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    ReputationTriggerSnapshot,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{
    Archive, DataForgeEntry, InputScope, Language, SourceInventory, SourceValueState,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::super::REPUTATION_TRIGGER_CONFIGURATIONS;
use super::{Route, TestResult, reputation_sandbox_fixture as fixture};
use crate::sdk_full_proof as full_proof;

const FIXTURE_OWNER_INDEX: u32 = 0;

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = archive(true)?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let built = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let session = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    session.with_reputation_triggers(CatalogRecordSelection::All, None, |snapshot| {
        compare(&snapshot, &catalog)?;
        Route::queried_rows(REPUTATION_TRIGGER_CONFIGURATIONS, &snapshot.rows, None)
    })?
}

fn compare(snapshot: &ReputationTriggerSnapshot<'_>, catalog: &Value) -> TestResult<()> {
    let [owner, empty] = snapshot.rows.as_slice() else {
        return Err("both physical owners, including the authored empty configuration".into());
    };
    assert_eq!(
        owner.owner_record_index().value(),
        Some(&FIXTURE_OWNER_INDEX),
        "the faction's physical owner index is shared with the canonical definition"
    );
    assert!(
        empty.sandbox_triggers().value().is_some_and(Vec::is_empty),
        "the empty trigger array remains an authored value"
    );
    let slots = owner
        .sandbox_triggers()
        .value()
        .ok_or("ordered trigger slots")?;
    let [_, null, _, _] = slots.as_slice() else {
        return Err("manual, null, linked and authored occurrences in original order".into());
    };
    assert_eq!(
        null.state(),
        SourceValueState::Null,
        "the authored null remains in place"
    );
    assert_eq!(
        null.origin().contains_encrypted_source(),
        Some(true),
        "the null pointer retains its own encrypted source marker"
    );
    let expected = catalog
        .pointer(REPUTATION_TRIGGER_CONFIGURATIONS)
        .ok_or("canonical trigger configurations")?;
    let table = json!({"availability": snapshot.availability, "rows": &snapshot.rows});
    assert_eq!(
        &table, expected,
        "the scope reuses canonical rows and original source uses for every trigger"
    );
    for pointer in [
        "/rows/0/sandboxTriggers/value/0/value/triggerOnInnocentsOnly",
        "/rows/0/sandboxTriggers/value/2/value/triggerRecord",
        "/rows/0/sandboxTriggers/value/3/value/authoredFields/values/1/value",
    ] {
        let field = table
            .pointer(pointer)
            .ok_or("source-backed trigger field")?;
        assert_origin(field, &snapshot.source_inventory)?;
    }
    Ok(())
}

fn assert_origin(field: &Value, inventory: &SourceInventory) -> TestResult<()> {
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(true)),
        "each trigger field retains the encrypted source marker"
    );
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("physical source uses")?;
    assert!(
        references
            .iter()
            .filter_map(Value::as_str)
            .any(|reference| {
                inventory.iter_uses().any(|used| {
                    used.id().as_str() == reference && used.scope() == InputScope::Payload
                })
            }),
        "the SDK retains the original payload use of this exact trigger field"
    );
    Ok(())
}

#[test]
fn faction_trigger_query_uses_physical_selection_without_opening_a_locked_locale() -> TestResult<()>
{
    let (_folder, archive) = archive(false)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_reputation_triggers(
        CatalogRecordSelection::RecordIndex(FIXTURE_OWNER_INDEX),
        None,
        |snapshot| -> TestResult<()> {
            let [owner] = snapshot.rows.as_slice() else {
                return Err("one exact physical faction owner".into());
            };
            assert_eq!(
                owner
                    .sandbox_triggers()
                    .origin()
                    .contains_encrypted_source(),
                Some(false),
                "the original trigger came from plaintext DataForge"
            );
            assert!(
                snapshot.coverage.unreadable_bodies.is_empty(),
                "the selected owner is readable with the locked locale present"
            );
            Ok(())
        },
    )?
}

fn archive(encrypted_dataforge: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let dataforge = TestEntry::zstd("Data/Game2.dcb", fixture::database().build());
    let dataforge = if encrypted_dataforge {
        dataforge.encrypted()
    } else {
        dataforge
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"fixture=Locked translation\n",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, encrypted_dataforge.then(p4kx_testkit::test_key))?;
    Ok((folder, archive))
}
