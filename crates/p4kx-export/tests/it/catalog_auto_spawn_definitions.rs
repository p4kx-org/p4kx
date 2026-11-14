// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use serde_json::{Value, json};

use super::auto_spawn_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[test]
fn auto_spawn_records_keep_typed_alternatives_and_authored_payloads() -> TestResult {
    let catalog = export()?;
    let spawn = at(
        &catalog,
        "/definitions/autoSpawns/rows/0/autoSpawnData/value",
    )?;
    assert_eq!(
        at(spawn, "/class/value/kind")?,
        "targetClass",
        "the direct class branch is distinct"
    );
    verify_spawn_settings(spawn)?;
    let sets = at(
        &catalog,
        "/definitions/autoSpawnSets/rows/0/autoSpawnData/value",
    )?;
    assert_eq!(
        at(sets, "/0/value/kind")?,
        "record",
        "reference alternative"
    );
    assert_eq!(at(sets, "/1/value/kind")?, "manual", "owned alternative");
    assert_eq!(at(sets, "/4/state")?, "null", "null ordered pointer slot");
    for slot in ["/0", "/5"] {
        let entry = at(sets, slot)?;
        assert_eq!(
            at(entry, "/value/record/value/guid/value")?,
            &json!(guid(fixture::SPAWN)),
            "repeated direct links keep both physical occurrences"
        );
    }
    verify_manual_classes(sets)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn verify_spawn_settings(spawn: &Value) -> TestResult {
    for (path, expected) in [
        ("/minGroupSize/value", json!(-1)),
        ("/maxConcurrentSpawns/value", json!(0)),
        ("/minSpawnDelay/value/rawBits", json!("80000000")),
        ("/maxSpawnDelay/value/rawBits", json!("3e000000")),
        ("/maxSpawnDelay/containsEncryptedSource", json!(true)),
        ("/onSpawnAssignment/value/kind", json!("scheduleArea")),
        (
            "/onSpawnAssignment/value/areaPositiveTags/value/tags/value/1/state",
            json!("null"),
        ),
        (
            "/onSpawnAssignment/value/areaNegativeTags/value/tags/value",
            json!([]),
        ),
        ("/inventoryItems/value/0/value/quantity/value", json!(0)),
        (
            "/inventoryItems/value/0/value/markAsMissionItem/value",
            json!(true),
        ),
        ("/entityTags/value/tags/value", json!([])),
    ] {
        assert_eq!(at(spawn, path)?, &expected, "original field at {path}");
    }
    Ok(())
}

fn verify_manual_classes(sets: &Value) -> TestResult {
    let archetype = at(sets, "/1/value/autoSpawnData/value/class/value")?;
    assert_eq!(
        at(archetype, "/kind")?,
        "archetype",
        "archetype alternative"
    );
    assert_eq!(
        at(archetype, "/archetype/value/0/value/weight/value/rawBits")?,
        "3e800000",
        "the authored archetype weight is exact"
    );
    assert_eq!(
        at(
            archetype,
            "/archetype/value/0/value/record/value/guid/value"
        )?,
        &json!(guid(super::ARCHETYPE)),
        "the actual archetype is linked"
    );
    let pool = at(sets, "/2/value/autoSpawnData/value/class/value")?;
    assert_eq!(at(pool, "/kind")?, "classPool", "class pool alternative");
    for slot in ["/classPool/value/0", "/classPool/value/1"] {
        let member = at(pool, slot)?;
        assert_eq!(
            at(member, "/value/guid/value")?,
            &json!(guid(super::POOL)),
            "the class pool Reference array preserves duplicates"
        );
    }
    let future = at(sets, "/3/value/autoSpawnData/value/class/value")?;
    assert_eq!(
        at(future, "/kind")?,
        "authored",
        "unrecognised derived variant"
    );
    assert_eq!(
        at(future, "/sourceType/value")?,
        "FixtureFutureSpawnClass",
        "original concrete type remains available"
    );
    assert!(
        at(future, "/authoredFields/roots")?
            .as_array()
            .is_some_and(|roots| !roots.is_empty()),
        "the unrecognised variant retains its authored payload"
    );
    Ok(())
}

#[test]
fn encounter_index_preserves_repeated_missing_and_null_set_slots() -> TestResult {
    let catalog = export()?;
    let row = at(&catalog, "/definitions/encounterIndices/rows/0")?;
    assert_eq!(
        at(row, "/guid/value")?,
        &json!(guid(fixture::ENCOUNTER)),
        "the encounter index is an independent Record"
    );
    let entry = at(row, "/encounters/value/0/value")?;
    assert_eq!(
        at(entry, "/encounterPreset/value")?,
        "FixturePreset",
        "authored Enum"
    );
    let slots = at(entry, "/autoSpawns/value")?;
    for slot in ["/0", "/2"] {
        assert_eq!(
            at(at(slots, slot)?, "/value/guid/value")?,
            &json!(guid(fixture::SET)),
            "the original set is linked at both positions"
        );
    }
    assert_eq!(
        at(slots, "/1/value/status")?,
        "dangling",
        "missing set target"
    );
    assert_eq!(at(slots, "/3/state")?, "null", "explicit null set target");
    Ok(())
}

fn export() -> TestResult<Value> {
    let (_directory, archive) =
        common::open(&fixture::extend(super::database()), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}
