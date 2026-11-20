// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ReputationRows, ReputationSelection,
    ReputationSnapshot, with_reputation_from,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{
    Archive, DataForgeEntry, FieldOrigin, InputScope, Language, SourceInventory,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::super::{REPUTATION_CONTEXTS, REPUTATION_PERKS, REPUTATION_TRIGGER_RECORDS};
use super::{Route, TestResult, reputation_fixture, reputation_sandbox_fixture as trigger_fixture};

#[path = "../it/reputation_perk_fixture.rs"]
mod perk_fixture;

pub(super) fn read() -> TestResult<[Route; 3]> {
    Ok([read_contexts()?, read_perks()?, read_triggers()?])
}

fn read_contexts() -> TestResult<Route> {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        reputation_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut selection = ReputationSelection::default();
    selection.contexts = Some(CatalogRecordSelection::All);
    compare(&archive, selection, |snapshot, catalog| {
        let rows = snapshot.contexts.ok_or("context result")?;
        let [context] = rows.rows.as_slice() else {
            return Err("one authored reputation context".into());
        };
        witness(
            REPUTATION_CONTEXTS,
            &rows,
            catalog,
            context.primary_scope_context_origin(),
            &snapshot.source_inventory,
        )
    })
}

fn read_perks() -> TestResult<Route> {
    let (_folder, archive) = synthetic(&perk_fixture::complete())?;
    let mut selection = ReputationSelection::default();
    selection.perks = Some(CatalogRecordSelection::All);
    compare(&archive, selection, |snapshot, catalog| {
        let rows = snapshot.perks.ok_or("perk list result")?;
        let [shared, empty] = rows.rows.as_slice() else {
            return Err("both physical lists, including the type-wide empty list".into());
        };
        assert!(
            empty.perks().value().is_some_and(Vec::is_empty),
            "a readable empty list remains a row"
        );
        let entries = shared
            .perks()
            .value()
            .ok_or("ordered original perk slots")?;
        let [first, ..] = entries.as_slice() else {
            return Err("the shared list contains a perk".into());
        };
        let original = first.value().ok_or("present original perk slot")?.perk();
        assert!(
            original.value().is_some(),
            "the owned perk pointer resolves inline"
        );
        witness(
            REPUTATION_PERKS,
            &rows,
            catalog,
            original.origin(),
            &snapshot.source_inventory,
        )
    })
}

fn read_triggers() -> TestResult<Route> {
    let (_folder, archive) = synthetic(&trigger_fixture::database())?;
    let mut selection = ReputationSelection::default();
    selection.trigger_records = Some(CatalogRecordSelection::All);
    compare(&archive, selection, |snapshot, catalog| {
        let rows = snapshot.trigger_records.ok_or("trigger Record result")?;
        let [record] = rows.rows.as_slice() else {
            return Err("one physical manual trigger Record".into());
        };
        assert!(
            snapshot.requested_languages.contains(&Language::English)
                && snapshot.available_languages.is_empty(),
            "the trigger-only request does not consult the provided English localization Entry"
        );
        let manual = record
            .trigger_manual_params()
            .value()
            .ok_or("authored manual slots")?;
        let [trigger] = manual.as_slice() else {
            return Err("one original manual trigger occurrence".into());
        };
        let original = trigger.value().ok_or("the original manual trigger")?;
        assert_eq!(
            original.source_type().value(),
            Some(&"SandboxTriggerManualParams"),
            "the SDK retains the original manual trigger subtype"
        );
        witness(
            REPUTATION_TRIGGER_RECORDS,
            &rows,
            catalog,
            record.trigger_manual_params().origin(),
            &snapshot.source_inventory,
        )
    })
}

fn synthetic(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"fixture=Synthetic fixture\n",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn compare(
    archive: &Archive,
    selection: ReputationSelection,
    inspect: impl FnOnce(ReputationSnapshot<'_>, &Value) -> TestResult<Route>,
) -> TestResult<Route> {
    let entry = DataForgeEntry::read(archive)?;
    let built = build_catalog_from(archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    with_reputation_from(
        archive,
        &entry,
        selection,
        CatalogLanguageSelection::AllAvailable,
        None,
        |snapshot| inspect(snapshot, &catalog),
    )?
}

fn witness<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    result: &ReputationRows<Row>,
    catalog: &Value,
    origin: &FieldOrigin,
    inventory: &SourceInventory,
) -> TestResult<Route> {
    let expected = catalog
        .pointer(dataset)
        .ok_or("canonical reputation table")?;
    let table = json!({"availability": result.availability, "rows": &result.rows});
    assert_eq!(
        &table, expected,
        "canonical rows and source references for {dataset}"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the original field comes from the encrypted DataForge Entry"
    );
    assert!(
        origin.source_set_complete()
            && inventory.iter_uses().any(|used| {
                used.scope() == InputScope::Payload && origin.source_uses_refs().contains(used.id())
            }),
        "the original payload proof is retained in the snapshot inventory"
    );
    Route::queried_rows(
        dataset,
        &result.rows,
        "encrypted synthetic producer parity available; installed producer parity open",
    )
}
