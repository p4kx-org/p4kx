// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::blueprint_taxonomy;
use super::record_targets::RecordTargets;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const DIRECTORY: &str = "libs/foundry/records/crafting";
const CATEGORY_TYPE: &str = "BlueprintCategoryRecord";
const BLUEPRINT_TYPE: &str = "CraftingBlueprintRecord";
const POOL_TYPE: &str = "BlueprintPoolRecord";
const SLOT_TYPE: &str = "FixtureBlueprintReward";
const REWARDS: &str = "blueprintRewards";
const BLUEPRINT: u128 = 0x1177_3101;
const DANGLING: u128 = 0x1177_3102;
const POOL: u128 = 0x1177_3103;

#[test]
fn blueprint_taxonomy_keeps_physical_rows_ordered_slots_and_markers() -> TestResult {
    with_queries(&database(), |parsed, scratch| {
        let targets = blueprint_targets(parsed)?;
        let categories = blueprint_taxonomy::collect_categories(parsed, scratch, None)?;
        let categories = serde_json::to_value(categories)?;
        let categories = rows(&categories)?;
        let [first, second, alias] = categories else {
            return Err("three original category Records".into());
        };
        for (row, index) in [(first, 1), (second, 0), (alias, 2)] {
            assert_eq!(
                row.pointer("/recordIndex/value"),
                Some(&json!(index)),
                "physical category order follows GUID, then physical Record index"
            );
            assert_eq!(
                row.pointer("/guid/containsEncryptedSource"),
                Some(&json!(true))
            );
            assert_eq!(row.as_object().map(serde_json::Map::len), Some(4));
        }

        let pools = blueprint_taxonomy::collect_pools(parsed, scratch, None, &targets)?;
        let definitions = serde_json::to_value(pools.definitions)?;
        let weights = serde_json::to_value(pools.weights)?;
        verify_pools(rows(&definitions)?, rows(&weights)?)
    })
}

fn verify_pools(definitions: &[Value], weights: &[Value]) -> TestResult {
    let [primary, alias] = definitions else {
        return Err("two physical pools of the same GUID".into());
    };
    let [primary_weight, alias_weight] = weights else {
        return Err("one canonical sum for each pool".into());
    };
    assert_eq!(primary.pointer("/guid/value"), alias.pointer("/guid/value"));
    for (definition, sum, index) in [(primary, primary_weight, 5), (alias, alias_weight, 6)] {
        assert_eq!(
            definition.pointer("/recordIndex/value"),
            Some(&json!(index))
        );
        assert_eq!(
            definition.pointer("/recordIndex/value"),
            sum.pointer("/pool/recordIndex/value"),
            "the sum references the corresponding physical pool"
        );
        assert_eq!(definition.get("guid"), sum.pointer("/pool/guid"));
        assert!(sum.pointer("/pool/inputRef").is_none());
        assert!(definition.get("additionalFields").is_none());
        assert_eq!(
            sum.pointer("/totalWeight/containsEncryptedSource"),
            Some(&json!(true))
        );
    }
    assert_eq!(
        primary_weight.pointer("/totalWeight/value"),
        Some(&json!({"type": "f32", "finite": 4.0})),
        "only the two positive finite source weights contribute"
    );
    assert_eq!(
        alias_weight.pointer("/totalWeight/value"),
        Some(&json!({"type": "f32", "finite": null, "nonFinite": "positiveInfinity"})),
        "the common projector retains the overflowed canonical f32 total"
    );
    verify_slots(primary)
}

fn verify_slots(pool: &Value) -> TestResult {
    let slots = pool
        .pointer("/blueprintRewards/value")
        .and_then(Value::as_array)
        .ok_or("selected reward slots")?;
    let [first, repeated, null_target, null_entry, dangling] = slots.as_slice() else {
        return Err("five preserved ordered reward slots".into());
    };
    assert_eq!(
        first.pointer("/value/blueprintRecord/value/targetRecordIndex/value"),
        repeated.pointer("/value/blueprintRecord/value/targetRecordIndex/value"),
        "both occurrences retain the actual physical target"
    );
    assert!(
        matches!(
            first
                .pointer("/value/blueprintRecord/value/targetRecordIndex/value")
                .and_then(Value::as_u64),
            Some(3 | 4)
        ),
        "the reference retains one physical Blueprint, including aliases"
    );
    for (slot, weight) in [(first, 2.5), (repeated, 1.5)] {
        assert_eq!(
            slot.pointer("/value/weight/value/finite"),
            Some(&json!(weight))
        );
        assert_eq!(
            slot.pointer("/value/weight/containsEncryptedSource"),
            Some(&json!(true))
        );
        assert!(slot.pointer("/value/additionalFields").is_none());
    }
    assert_eq!(
        null_target.pointer("/value/blueprintRecord/state"),
        Some(&json!("null"))
    );
    assert_eq!(null_entry.get("state"), Some(&json!("null")));
    assert_eq!(
        dangling.pointer("/value/blueprintRecord/value/status"),
        Some(&json!("dangling"))
    );
    assert_eq!(
        dangling.pointer("/value/weight/value"),
        Some(&json!({"type": "f32", "finite": null, "nonFinite": "naN"}))
    );
    Ok(())
}

#[test]
fn blueprints_require_links_to_close_over_the_reduced_physical_targets() -> TestResult {
    with_queries(&database(), |parsed, scratch| {
        let outcome =
            blueprint_taxonomy::collect_pools(parsed, scratch, None, &RecordTargets::default());
        assert!(
            matches!(
                outcome,
                Err(ReducedError::UnresolvedRecordLink {
                    path: blueprint_taxonomy::POOLS,
                    index: 3 | 4
                })
            ),
            "a resolved Blueprint cannot point to a missing reduced physical row"
        );
        Ok(())
    })
}

#[test]
fn pool_inventory_retains_absent_members_but_refuses_invalid_members() -> TestResult {
    let absent = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(POOL_TYPE))
        .record(record(POOL, POOL_TYPE, "blueprintrewards", "undeclared"));
    with_queries(&absent, |parsed, scratch| {
        let reduced =
            blueprint_taxonomy::collect_pools(parsed, scratch, None, &RecordTargets::default())?;
        let definition = serde_json::to_value(reduced.definitions)?;
        let [row] = rows(&definition)? else {
            return Err("the pool exists without a reward declaration".into());
        };
        assert_eq!(
            row.pointer("/blueprintRewards/state"),
            Some(&json!("absent"))
        );
        Ok(())
    })?;
    let invalid = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(POOL_TYPE).array(REWARDS, TestType::Bool))
        .record(
            record(POOL, POOL_TYPE, "blueprintrewards", "wrong_storage")
                .value(REWARDS, TestValue::Array(vec![TestValue::Bool(true)])),
        );
    with_queries(&invalid, |parsed, scratch| {
        let outcome =
            blueprint_taxonomy::collect_pools(parsed, scratch, None, &RecordTargets::default());
        assert!(
            matches!(
                outcome,
                Err(ReducedError::IncompleteScope(blueprint_taxonomy::POOLS))
            ),
            "a wrong reward storage type cannot masquerade as authored absence"
        );
        Ok(())
    })
}

fn rows(table: &Value) -> Result<&[Value], &'static str> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("reduced taxonomy rows")
}

fn blueprint_targets(parsed: &ParsedCatalogQueries<'_>) -> Result<RecordTargets, ReducedError> {
    parsed.with_blueprints(CatalogRecordSelection::All, None, |source| {
        let mut targets = RecordTargets::default();
        for blueprint in source.definitions() {
            targets.insert("definitions.blueprints", blueprint.header())?;
        }
        Ok(targets)
    })?
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

fn database() -> DataForgeBuilder {
    let slots = TestStruct::new(SLOT_TYPE)
        .field("blueprintRecord", TestType::Reference)
        .field("weight", TestType::Single);
    let pool = TestStruct::new(POOL_TYPE)
        .array(REWARDS, TestType::StrongPointer(SLOT_TYPE.to_owned()))
        .field("futureNumber", TestType::Single);
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(CATEGORY_TYPE))
        .structure(TestStruct::new(BLUEPRINT_TYPE))
        .structure(slots)
        .structure(pool)
        .record(record(2, CATEGORY_TYPE, "blueprintcategories", "second"))
        .record(record(1, CATEGORY_TYPE, "blueprintcategories", "first"))
        .record(record(2, CATEGORY_TYPE, "blueprintcategories", "alias"))
        .record(record(
            BLUEPRINT,
            BLUEPRINT_TYPE,
            "blueprints/crafting",
            "base",
        ))
        .record(record(
            BLUEPRINT,
            BLUEPRINT_TYPE,
            "blueprints/crafting",
            "alias",
        ))
        .record(
            record(POOL, POOL_TYPE, "blueprintrewards", "primary")
                .value("futureNumber", TestValue::Single(9.0))
                .value(
                    REWARDS,
                    TestValue::Array(vec![
                        slot(BLUEPRINT, 2.5),
                        slot(BLUEPRINT, 1.5),
                        slot(0, -3.0),
                        TestValue::Null,
                        slot(DANGLING, f32::NAN),
                    ]),
                ),
        )
        .record(record(POOL, POOL_TYPE, "blueprintrewards", "alias").value(
            REWARDS,
            TestValue::Array(vec![slot(BLUEPRINT, f32::MAX), slot(BLUEPRINT, f32::MAX)]),
        ))
}

fn record(guid: u128, kind: &str, family: &str, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.{name}"),
        format!("{DIRECTORY}/{family}/{name}.xml"),
    )
}

fn slot(blueprint: u128, weight: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(SLOT_TYPE)
            .value("blueprintRecord", TestValue::Reference(blueprint))
            .value("weight", TestValue::Single(weight)),
    )
}
