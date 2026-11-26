// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::blueprints;
use super::crafting_global;
use super::dismantle_processes;
use super::record_targets::RecordTargets;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::guid_text::CatalogGuid;

#[path = "crafting_dependencies_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reduced_dismantle_process_closes_a_blueprint_link_to_the_canonical_physical_row() -> TestResult {
    with_queries(&fixture::database(), |parsed, scratch| {
        let processes = dismantle_processes::collect(parsed, scratch, None)?;
        let blueprints = blueprints::collect(
            parsed,
            scratch,
            None,
            &mut SelectedTexts::default(),
            &processes,
        )?;
        let blueprint_rows = serde_json::to_value(blueprints.presentations)?;
        let link = blueprint_rows
            .pointer("/rows/0/dismantle/value/processGuid")
            .and_then(Value::as_str)
            .ok_or("one canonical dismantle process link")?;
        let index = processes.unique_record(CatalogGuid(link.parse()?))?;
        let export = serde_json::to_value(processes.definitions)?;
        let [process] = rows(&export)? else {
            return Err("one original process".into());
        };
        assert_eq!(process.pointer("/guid/value"), Some(&json!(link)));
        assert_eq!(process.pointer("/recordIndex/value"), Some(&json!(index)));
        assert_eq!(index, 0);
        verify_process(process)
    })
}

fn verify_process(process: &Value) -> TestResult {
    let configuration = process
        .pointer("/blueprint/value/processSpecificData/value")
        .ok_or("original process configuration")?;
    assert_eq!(
        configuration.get("efficiency"),
        Some(
            &json!({"state":"present", "value":{"type":"f64", "finite":0.625}, "containsEncryptedSource":true})
        )
    );
    assert_eq!(
        configuration.get("timeSeconds"),
        Some(&json!({"state":"present", "value":"75", "containsEncryptedSource":true}))
    );
    assert_eq!(configuration.as_object().map(serde_json::Map::len), Some(2));
    assert!(process.get("authoredFields").is_none());
    Ok(())
}

#[test]
fn duplicate_physical_dismantle_processes_remain_rows_without_a_blueprint_choice() -> TestResult {
    with_queries(&fixture::duplicate(), |parsed, scratch| {
        let processes = dismantle_processes::collect(parsed, scratch, None)?;
        let blueprints = blueprints::collect(
            parsed,
            scratch,
            None,
            &mut SelectedTexts::default(),
            &processes,
        )?;
        let blueprint = serde_json::to_value(blueprints.presentations)?;
        assert_eq!(
            blueprint.pointer("/rows/0/dismantle/state"),
            Some(&json!("absent"))
        );
        let records = serde_json::to_value(&processes.definitions)?;
        let [primary, alias] = rows(&records)? else {
            return Err("both physical process aliases".into());
        };
        assert_eq!(primary.pointer("/guid/value"), alias.pointer("/guid/value"));
        assert_ne!(
            primary.pointer("/recordIndex/value"),
            alias.pointer("/recordIndex/value")
        );
        let guid = primary
            .pointer("/guid/value")
            .and_then(Value::as_str)
            .ok_or("known GUID")?;
        assert!(
            matches!(
                processes.unique_record(CatalogGuid(guid.parse()?)),
                Err(ReducedError::IncompleteScope(dismantle_processes::PATH))
            ),
            "a same-GUID alias cannot substitute for the physical process selection"
        );
        Ok(())
    })
}

#[test]
fn reduced_crafting_globals_keep_all_ordered_resource_slots_and_require_closure() -> TestResult {
    with_queries(&fixture::database(), |parsed, scratch| {
        let targets =
            parsed.with_resource_types(CatalogRecordSelection::All, None, |snapshot| {
                let mut targets = RecordTargets::default();
                for row in snapshot.rows() {
                    targets.insert("definitions.resourceTypes", row.source())?;
                }
                Ok::<_, ReducedError>(targets)
            })??;
        let rows =
            serde_json::to_value(crafting_global::collect(parsed, scratch, None, &targets)?)?;
        verify_globals(&rows)?;
        let missing = crafting_global::collect(parsed, scratch, None, &RecordTargets::default());
        assert!(
            matches!(
                missing,
                Err(ReducedError::UnresolvedRecordLink {
                    path: crafting_global::PATH,
                    index: 4
                })
            ),
            "the exact ResourceType must be in the selected reduced target table"
        );
        Ok(())
    })
}

fn verify_globals(table: &Value) -> TestResult {
    let [first, alias] = rows(table)? else {
        return Err("two original global configurations".into());
    };
    assert_eq!(first.pointer("/guid/value"), alias.pointer("/guid/value"));
    assert_ne!(
        first.pointer("/recordIndex/value"),
        alias.pointer("/recordIndex/value")
    );
    assert_eq!(
        alias.pointer("/dismantleBlacklistResources/value"),
        Some(&json!([]))
    );
    let slots = first
        .pointer("/dismantleBlacklistResources/value")
        .and_then(Value::as_array)
        .ok_or("ordered blacklist")?;
    let [first_slot, null, repeated, dangling, foreign] = slots.as_slice() else {
        return Err("five original resource exclusion slots".into());
    };
    assert_eq!(first_slot, repeated, "repeated original slots are retained");
    assert_eq!(
        first_slot.pointer("/value/targetRecordIndex/value"),
        Some(&json!(4))
    );
    assert_eq!(
        first_slot.pointer("/containsEncryptedSource"),
        Some(&json!(true))
    );
    assert_eq!(null.pointer("/state"), Some(&json!("null")));
    assert_eq!(dangling.pointer("/value/status"), Some(&json!("dangling")));
    assert_eq!(
        foreign.pointer("/value/targetType/value"),
        Some(&json!("FixtureForeign"))
    );
    assert_eq!(
        foreign.pointer("/value/targetRecordIndex/value"),
        Some(&json!(6))
    );
    for field in [
        "defaultBlueprintSelection",
        "dismantleBlacklistEntityClasses",
        "additionalFields",
    ] {
        assert!(first.get(field).is_none());
    }
    Ok(())
}

fn rows(table: &Value) -> Result<&[Value], &'static str> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("staged crafting rows")
}

fn with_queries(
    database: &DataForgeBuilder,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, scratch.path())
}
