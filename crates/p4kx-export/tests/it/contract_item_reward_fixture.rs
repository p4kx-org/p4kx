// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const PARENT: u128 = 0x5210;
pub(super) const SUB: u128 = 0x5211;
pub(super) const FALLBACK: u128 = 0x5212;

const ENTITY_ID: u128 = 0x5213;
const POOL_ID: u128 = 0x5214;
const GENERATOR: &str = "ContractGenerator";
const HANDLER: &str = "FixtureItemHandler";
const CONTRACT: &str = "FixtureItemContract";
const SUB_CONTRACT: &str = "FixtureItemSubContract";
const BLOCK: &str = "FixtureItemResultBlock";
const BASE: &str = "ContractResult";
const ITEM: &str = "ContractResult_Item";
const BLUEPRINT: &str = "BlueprintRewards";
const ENTITY: &str = "EntityClassDefinition";
const POOL: &str = "BlueprintPoolRecord";
const PROPERTY: &str = "MissionProperty";
const NOTIFICATION: &str = "FixtureNotification";
const RESULTS: &str = "contractResults";
const TARGET: &str = "targetLocation";
const WHOLE: &str = "MissionProperty.FixtureWhole";
const INSTANCE: &str = "FixturePrimary";
const OUTSIDE: &str = "FixtureUnowned";

pub(super) fn database() -> Vec<u8> {
    structures()
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(TestRecord::new(
            0x5215,
            PROPERTY,
            WHOLE,
            "fixture/whole_property.xml",
        ))
        .record(TestRecord::new(
            ENTITY_ID,
            ENTITY,
            "EntityClassDefinition.Fixture",
            "fixture/item.xml",
        ))
        .record(TestRecord::new(
            POOL_ID,
            POOL,
            "BlueprintPoolRecord.Fixture",
            "fixture/pool.xml",
        ))
        .record(generator())
        .instance(OUTSIDE, TestInstance::new(PROPERTY))
        .build()
}

fn structures() -> [TestStruct; 12] {
    [
        TestStruct::new(PROPERTY).field("name", TestType::String),
        TestStruct::new(ENTITY),
        TestStruct::new(POOL),
        TestStruct::new(GENERATOR)
            .array("generators", class(HANDLER))
            .array("locations", class(PROPERTY)),
        TestStruct::new(HANDLER).array("contracts", class(CONTRACT)),
        TestStruct::new(CONTRACT)
            .field("id", TestType::Guid)
            .field(RESULTS, class(BLOCK))
            .array("subContracts", class(SUB_CONTRACT)),
        TestStruct::new(SUB_CONTRACT)
            .field("id", TestType::Guid)
            .field(RESULTS, class(BLOCK)),
        TestStruct::new(BLOCK).array(RESULTS, TestType::StrongPointer(BASE.to_owned())),
        TestStruct::new(BASE).array("missionResults", TestType::Bool),
        TestStruct::new(ITEM)
            .parent(BASE)
            .field("entityClass", TestType::ReferenceTo(ENTITY.to_owned()))
            .field("amount", TestType::Int32)
            .field("awardOnlyToMissionOwner", TestType::Bool)
            .field("sendToPlayerHomeLocation", TestType::Bool)
            .field(TARGET, TestType::WeakPointer(PROPERTY.to_owned()))
            .field("notification", class(NOTIFICATION)),
        TestStruct::new(BLUEPRINT)
            .parent(BASE)
            .field("blueprintPool", TestType::ReferenceTo(POOL.to_owned()))
            .field("chance", TestType::Single),
        TestStruct::new(NOTIFICATION).field("enabled", TestType::Bool),
    ]
}

fn generator() -> TestRecord {
    let parent_items = [
        item(TestValue::pointer(INSTANCE), -4, true),
        blueprint(),
        item(TestValue::pointer(INSTANCE), -4, true),
        item(TestValue::Null, 2, true),
        item(TestValue::pointer(OUTSIDE), 3, false),
    ];
    let sub = TestInstance::new(SUB_CONTRACT)
        .value("id", TestValue::Guid(SUB))
        .value(RESULTS, block([item(TestValue::pointer(WHOLE), 0, false)]));
    let fallback = TestInstance::new(SUB_CONTRACT).value("id", TestValue::Guid(FALLBACK));
    let parent = TestInstance::new(CONTRACT)
        .value("id", TestValue::Guid(PARENT))
        .value(RESULTS, block(parent_items))
        .value("subContracts", array([sub, fallback]));
    let handler = TestInstance::new(HANDLER).value("contracts", array([parent]));
    TestRecord::new(
        0x5216,
        GENERATOR,
        "ContractGenerator.FixtureRewards",
        "libs/foundry/records/contracts/contractgenerator/fixture_rewards.xml",
    )
    .value(
        "locations",
        array([
            TestInstance::new(PROPERTY).value("name", TestValue::text("same display name")),
            TestInstance::new(PROPERTY)
                .named(INSTANCE)
                .value("name", TestValue::text("same display name")),
        ]),
    )
    .value("generators", array([handler]))
}

fn item(target: TestValue, amount: i32, owner_only: bool) -> TestInstance {
    TestInstance::new(ITEM)
        .value("missionResults", flags([owner_only, false, true]))
        .value("entityClass", TestValue::Reference(ENTITY_ID))
        .value("amount", TestValue::Int32(amount))
        .value("awardOnlyToMissionOwner", TestValue::Bool(owner_only))
        .value("sendToPlayerHomeLocation", TestValue::Bool(!owner_only))
        .value(TARGET, target)
        .value(
            "notification",
            TestValue::Struct(
                TestInstance::new(NOTIFICATION).value("enabled", TestValue::Bool(true)),
            ),
        )
}

fn blueprint() -> TestInstance {
    TestInstance::new(BLUEPRINT)
        .value("missionResults", flags([true]))
        .value("blueprintPool", TestValue::Reference(POOL_ID))
        .value("chance", TestValue::Single(1.0))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn array<const N: usize>(values: [TestInstance; N]) -> TestValue {
    TestValue::Array(values.into_iter().map(TestValue::Struct).collect())
}

fn block<const N: usize>(results: [TestInstance; N]) -> TestValue {
    TestValue::Struct(TestInstance::new(BLOCK).value(RESULTS, array(results)))
}

fn flags<const N: usize>(values: [bool; N]) -> TestValue {
    TestValue::Array(values.into_iter().map(TestValue::Bool).collect())
}
