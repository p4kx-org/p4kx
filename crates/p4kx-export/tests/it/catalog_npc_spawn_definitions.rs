// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[path = "catalog_auto_spawn_definitions.rs"]
mod auto_spawn_definitions;
#[path = "catalog_auto_spawn_fixture.rs"]
mod auto_spawn_fixture;

const ARCHETYPE: u128 = 0x1084_0001;
const POOL: u128 = 0x1084_0002;
const EMPTY_POOL: u128 = 0x1084_0003;
const MISSING: u128 = 0x1084_00ff;

#[test]
fn npc_definitions_keep_weighted_occurrences_and_provenance() -> TestResult {
    let catalog = export(None)?;
    let pool = at(&catalog, "/definitions/npcClassPools/rows/0")?;
    assert_eq!(at(pool, "/guid/value")?, &json!(guid(POOL)));
    assert_eq!(
        at(pool, "/npcClasses/value/0/value/weight/value/rawBits")?,
        "80000000",
        "negative zero retains its exact Single bits"
    );
    assert_eq!(
        at(pool, "/npcClasses/value/1/value/weight/value/rawBits")?,
        "3fa00000",
        "the second occurrence retains its own weight"
    );
    for (path, expected) in [
        ("/npcClasses/value/0/value/npcClass/state", "null"),
        (
            "/npcClasses/value/1/value/npcClass/value/status",
            "dangling",
        ),
    ] {
        assert_eq!(at(pool, path)?, expected);
    }
    assert_eq!(
        at(pool, "/npcClasses/value/1/value/npcClass/value/guid/value")?,
        &json!(guid(MISSING))
    );
    for path in [
        "/npcClasses/value/0/value/weight/containsEncryptedSource",
        "/npcClasses/value/1/value/npcClass/value/guid/containsEncryptedSource",
    ] {
        assert_eq!(at(pool, path)?, true);
    }
    assert_eq!(
        at(
            &catalog,
            "/definitions/npcClassPools/rows/1/npcClasses/value"
        )?,
        &json!([]),
        "the independent empty pool survives"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

#[test]
fn npc_archetype_reuses_pool_identity_across_ordered_branches() -> TestResult {
    let filter = Filter::new("*/npc_fixture/archetypes/*");
    let catalog = export(Some(&filter))?;
    let archetype = at(&catalog, "/definitions/npcArchetypes/rows/0")?;
    assert_eq!(at(archetype, "/guid/value")?, &json!(guid(ARCHETYPE)));
    assert_eq!(at(&catalog, "/definitions/npcClassPools/rows")?, &json!([]));
    for path in [
        "/identity/value/0/value/npcPools/value/0",
        "/identity/value/0/value/npcPools/value/1",
        "/identity/value/1/value/npcPools/value/0",
    ] {
        assert_eq!(
            at(at(archetype, path)?, "/value/guid/value")?,
            &json!(guid(POOL)),
            "each authored occurrence retains the shared pool GUID"
        );
    }
    assert_eq!(
        at(archetype, "/identity/value/0/value/identity/state")?,
        "null"
    );
    assert_eq!(
        at(archetype, "/identity/value/1/value/identity/value/status")?,
        "dangling"
    );
    let references = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved reference rows")?;
    assert!(references.iter().any(|row| {
        row["status"] == "excludedByFilter" && row["targetGuid"]["value"] == guid(POOL)
    }));
    Ok(())
}

fn export(filter: Option<&Filter>) -> TestResult<Value> {
    let (_directory, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("FactionIdentity"))
        .structure(
            TestStruct::new("NPCClassPoolEntry")
                .field("weight", TestType::Single)
                .field("npcClass", reference("EntityClassDefinition")),
        )
        .structure(
            TestStruct::new("NPCClassPool")
                .array("npcClasses", TestType::Class("NPCClassPoolEntry".into())),
        )
        .structure(
            TestStruct::new("NPCIdentityToArchetype")
                .field("identity", reference("FactionIdentity"))
                .array("npcPools", reference("NPCClassPool")),
        )
        .structure(
            TestStruct::new("NPCArchetype")
                .array("identity", TestType::Class("NPCIdentityToArchetype".into())),
        )
        .record(
            record(EMPTY_POOL, "NPCClassPool", "pools/empty")
                .value("npcClasses", TestValue::Array(Vec::new())),
        )
        .record(record(POOL, "NPCClassPool", "pools/shared").value(
            "npcClasses",
            TestValue::Array(vec![
                pool_entry(-0.0, TestValue::Null),
                pool_entry(1.25, TestValue::Reference(MISSING)),
            ]),
        ))
        .record(
            record(ARCHETYPE, "NPCArchetype", "archetypes/primary").value(
                "identity",
                TestValue::Array(vec![
                    identity_branch(TestValue::Null, vec![TestValue::Reference(POOL); 2]),
                    identity_branch(
                        TestValue::Reference(MISSING),
                        vec![TestValue::Reference(POOL)],
                    ),
                ]),
            ),
        )
}

fn record(guid: u128, source_type: &str, suffix: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Fixture"),
        format!("libs/foundry/records/npc_fixture/{suffix}.xml"),
    )
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn pool_entry(weight: f32, npc_class: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new("NPCClassPoolEntry")
            .value("weight", TestValue::Single(weight))
            .value("npcClass", npc_class),
    )
}

fn identity_branch(identity: TestValue, pools: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("NPCIdentityToArchetype")
            .value("identity", identity)
            .value("npcPools", TestValue::Array(pools)),
    )
}
