// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, TestEntity, TestEntry, TestInstance, TestRecord, TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::record_targets::RecordTargets;
use super::resource_components;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

#[path = "../../tests/it/catalog_resource_component_fixture.rs"]
mod fixture;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn keeps_resource_states_and_typed_deltas_without_raw_arenas() -> TestResult {
    with_queries(|parsed, scratch| {
        let targets = read_targets(parsed)?;
        let output = resource_components::collect(parsed, scratch, None, &targets)?;
        let wire = serde_json::to_string(&output)?;
        let table: Value = serde_json::from_str(&wire)?;
        let rows = table
            .get("rows")
            .and_then(Value::as_array)
            .ok_or("resource components")?;
        let [component, specialist, alias] = rows.as_slice() else {
            return Err("three selected physical resource components".into());
        };
        assert_component_order(component, specialist, alias)?;
        let state = at(component, "/states/value/0/value")?;
        assert_present(state, "/sourcePosition", &json!(0))?;
        assert_present(state, "/name", &json!("FixtureEnabled"))?;
        assert_power_range(state, 0, &json!({"type": "f32", "finite": 0.0}), false)?;
        let alternate = at(alias, "/states/value/0/value")?;
        assert_present(alternate, "/name", &json!("FixtureOverride"))?;
        let infinite = json!({"type": "f32", "finite": null, "nonFinite": "negativeInfinity"});
        assert_power_range(alternate, -5, &infinite, true)?;
        let deltas = at(state, "/deltas/value")?
            .as_array()
            .ok_or("ordered resource deltas")?;
        assert_delta_slots(deltas)?;
        for discarded in [
            "sourceUsesRefs",
            "rawBits",
            "additionalFields",
            "signatureParams",
            "isRelay",
        ] {
            assert!(
                !wire.contains(discarded),
                "the reduced scope excludes {discarded}"
            );
        }
        Ok(())
    })
}

#[test]
fn present_resource_composition_target_must_be_in_selected_resource_definitions() -> TestResult {
    with_queries(|parsed, scratch| {
        let error =
            resource_components::collect(parsed, scratch, None, &RecordTargets::default()).err();
        assert!(
            matches!(
                error,
                Some(ReducedError::UnresolvedRecordLink {
                    path: resource_components::PATH,
                    index: 2,
                })
            ),
            "the resolved composition points to a physical ResourceType that must be emitted"
        );
        Ok(())
    })
}

fn assert_component_order(component: &Value, specialist: &Value, alias: &Value) -> TestResult {
    for (row, record, ordinal) in [(component, 0, 1), (specialist, 1, 1), (alias, 3, 0)] {
        assert_present(row, "/ownerRecordIndex", &json!(record))?;
        assert_present(row, "/componentOrdinal", &json!(ordinal))?;
    }
    assert_eq!(
        component.get("ownerGuid"),
        alias.get("ownerGuid"),
        "equal GUIDs do not merge separate physical components"
    );
    Ok(())
}

fn assert_delta_slots(deltas: &[Value]) -> TestResult {
    let [
        generation,
        empty,
        first_conversion,
        consumption,
        second_conversion,
        storage,
        reflection,
        unknown,
    ] = deltas
    else {
        return Err("original eight delta positions".into());
    };
    assert_eq!(empty.get("state"), Some(&json!("null")));
    assert_eq!(empty.get("value"), Some(&Value::Null));
    for (position, kind, slot) in [
        (0, "generation", generation),
        (2, "conversion", first_conversion),
        (3, "consumption", consumption),
        (4, "conversion", second_conversion),
        (5, "storage", storage),
        (6, "networkReflection", reflection),
        (7, "unknownDelta", unknown),
    ] {
        let value = at(slot, "/value")?;
        assert_eq!(value.get("kind"), Some(&json!(kind)), "delta at {position}");
        assert_present(value, "/sourcePosition", &json!(position))?;
    }
    assert_present(
        at(generation, "/value")?,
        "/generation/value/resourceAmountPerSecond/value/units",
        &json!({"type":"int32", "value":17}),
    )?;
    assert_present(
        at(storage, "/value")?,
        "/generation/value/resourceAmountPerSecond/value/microResourceUnits",
        &json!({"type":"int32", "value":-8}),
    )?;
    assert_present(
        at(reflection, "/value")?,
        "/resource",
        &json!("FixtureNewResource"),
    )?;
    assert_present(
        at(unknown, "/value")?,
        "/sourceType",
        &json!("FixtureFutureDelta"),
    )?;
    assert_conversion(first_conversion, 4, 1.25, "dangling", None)?;
    assert_conversion(second_conversion, 8, 3.5, "present", Some(2))
}

fn assert_conversion(
    slot: &Value,
    consumed: i32,
    generated: f32,
    target_status: &str,
    target_position: Option<u32>,
) -> TestResult {
    let conversion = at(slot, "/value")?;
    assert_present(conversion, "/consumption/value/resource", &json!("Power"))?;
    assert_present(conversion, "/generation/value/resource", &json!("Coolant"))?;
    assert_present(
        conversion,
        "/consumption/value/resourceAmountPerSecond/value/units",
        &json!({"type":"int32", "value":consumed}),
    )?;
    assert_present(
        conversion,
        "/generation/value/resourceAmountPerSecond/value/standardResourceUnits",
        &json!({"type":"f32", "finite":generated}),
    )?;
    let ratio = at(
        conversion,
        "/generatedComposition/value/values/value/0/value",
    )?;
    assert_present(ratio, "/ratio", &json!({"type":"f32", "finite":0.5}))?;
    let target = at(ratio, "/containerResource/value")?;
    assert_eq!(target.get("status"), Some(&json!(target_status)));
    assert_eq!(
        target.pointer("/targetRecordIndex/value"),
        Some(&json!(target_position))
    );
    Ok(())
}

fn assert_power_range(state: &Value, start: i32, modifier: &Value, register: bool) -> TestResult {
    let range = at(state, "/powerRanges/value/low/value")?;
    assert_present(range, "/start", &json!(start))?;
    assert_present(range, "/modifier", modifier)?;
    assert_present(range, "/registerRange", &json!(register))
}

fn at<'read>(row: &'read Value, pointer: &str) -> TestResult<&'read Value> {
    row.pointer(pointer)
        .ok_or_else(|| format!("missing field {pointer}").into())
}

fn assert_present(row: &Value, pointer: &str, expected: &Value) -> TestResult {
    let cell = at(row, pointer)?;
    assert_eq!(cell.get("state"), Some(&json!("present")), "{pointer}");
    assert_eq!(cell.get("value"), Some(expected), "{pointer}");
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(true)),
        "{pointer}"
    );
    Ok(())
}

fn read_targets(parsed: &ParsedCatalogQueries<'_>) -> TestResult<RecordTargets> {
    let targets = parsed.with_resource_types(CatalogRecordSelection::All, None, |snapshot| {
        let mut targets = RecordTargets::default();
        for row in snapshot.rows() {
            targets.insert(resource_components::PATH, row.source())?;
        }
        Ok::<_, ReducedError>(targets)
    })??;
    Ok(targets)
}

fn with_queries(
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let database = fixture::database().record(resource_alias());
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, scratch.path())
}

fn resource_alias() -> TestRecord {
    let low = TestInstance::new("ResourceRangeParams")
        .value("start", TestValue::Int32(-5))
        .value("modifier", TestValue::Single(f32::NEG_INFINITY))
        .value("registerRange", TestValue::Bool(true));
    let ranges = TestInstance::new("PowerRangeParams").value("low", TestValue::Struct(low));
    let state = TestInstance::new("ItemResourceState")
        .value("name", TestValue::text("FixtureOverride"))
        .value("powerRanges", TestValue::Struct(ranges));
    let component = TestInstance::new("ItemResourceComponentParams")
        .value("states", TestValue::Array(vec![TestValue::Struct(state)]));
    TestEntity::new(
        fixture::OWNER,
        "FixtureResourceAlias",
        "libs/foundry/records/entities/scitem/ships/resource/fixture_alias.xml",
    )
    .component(TestValue::Struct(component))
    .record()
}
