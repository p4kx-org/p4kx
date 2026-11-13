// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const SPAWN: u128 = 0x1084_0101;
pub(super) const SET: u128 = 0x1084_0102;
pub(super) const ENCOUNTER: u128 = 0x1084_0103;
pub(super) const MISSING: u128 = 0x1084_01ff;

const SPAWN_DATA: &str = "AutoSpawnData";
const SPAWN_CLASS: &str = "AutoSpawnClass";
const SET_ENTRY: &str = "AutoSpawnsData";
const ASSIGNMENT: &str = "OnSpawnAssignment";
const PRESET: &str = "EncounterPreset";
const INVENTORY: &str = "SpawnSettingsInventoryItem";

pub(super) fn extend(database: DataForgeBuilder) -> DataForgeBuilder {
    encounter_schema(spawn_schema(class_schema(database)))
        .record(record(SPAWN, "AutoSpawn", "spawns/direct").value(
            "autoSpawnData",
            spawn_data(class("AutoSpawnTargetClass").value("npcClass", TestValue::Null)),
        ))
        .record(
            record(SET, "AutoSpawns", "sets/ordered").value(
                "autoSpawnData",
                array([
                    class("AutoSpawnsDataRef")
                        .value("record", TestValue::Reference(SPAWN))
                        .into_value(),
                    manual(archetype_class()),
                    manual(pool_class()),
                    manual(
                        class("FixtureFutureSpawnClass").value("fixtureValue", TestValue::Int32(7)),
                    ),
                    TestValue::Null,
                    class("AutoSpawnsDataRef")
                        .value("record", TestValue::Reference(SPAWN))
                        .into_value(),
                ]),
            ),
        )
        .record(
            record(ENCOUNTER, "EncounterIndex", "encounters/fixture").value(
                "encounters",
                array([class("EncounterIndexEntry")
                    .value("encounterPreset", TestValue::text("FixturePreset"))
                    .value(
                        "autoSpawns",
                        array([
                            TestValue::Reference(SET),
                            TestValue::Reference(MISSING),
                            TestValue::Reference(SET),
                            TestValue::Null,
                        ]),
                    )
                    .into_value()]),
            ),
        )
}

fn class_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new(SPAWN_CLASS))
        .structure(
            TestStruct::new("AutoSpawnTargetClass")
                .parent(SPAWN_CLASS)
                .field("npcClass", reference("EntityClassDefinition")),
        )
        .structure(
            TestStruct::new("NPCArchetypeWeight")
                .field("weight", TestType::Single)
                .field("record", reference("NPCArchetype")),
        )
        .structure(
            TestStruct::new("AutoSpawnArchetype")
                .parent(SPAWN_CLASS)
                .array("archetype", inline("NPCArchetypeWeight")),
        )
        .structure(
            TestStruct::new("AutoSpawnClassPool")
                .parent(SPAWN_CLASS)
                .array("classPool", reference("NPCClassPool")),
        )
        .structure(
            TestStruct::new("FixtureFutureSpawnClass")
                .parent(SPAWN_CLASS)
                .field("fixtureValue", TestType::Int32),
        )
}

fn spawn_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("TagList").array("tags", reference("Tag")))
        .structure(
            TestStruct::new(ASSIGNMENT)
                .field("areaPositiveTags", inline("TagList"))
                .field("areaNegativeTags", inline("TagList")),
        )
        .structure(TestStruct::new("ScheduleAreaAssignment").parent(ASSIGNMENT))
        .structure(
            TestStruct::new(INVENTORY)
                .field("entityClass", reference("EntityClassDefinition"))
                .field("quantity", TestType::Int32)
                .field("amountInventoriesToFill", TestType::Int32)
                .field("markAsMissionItem", TestType::Bool),
        )
        .structure(spawn_data_schema())
        .structure(TestStruct::new("AutoSpawn").field("autoSpawnData", inline(SPAWN_DATA)))
        .structure(TestStruct::new(SET_ENTRY))
        .structure(
            TestStruct::new("AutoSpawnsDataRef")
                .parent(SET_ENTRY)
                .field("record", reference("AutoSpawn")),
        )
        .structure(
            TestStruct::new("AutoSpawnsDataManual")
                .parent(SET_ENTRY)
                .field("autoSpawnData", inline(SPAWN_DATA)),
        )
        .structure(TestStruct::new("AutoSpawns").array("autoSpawnData", pointer(SET_ENTRY)))
}

fn spawn_data_schema() -> TestStruct {
    TestStruct::new(SPAWN_DATA)
        .field("class", pointer(SPAWN_CLASS))
        .field("identity", reference("FactionIdentity"))
        .field("initialActivity", TestType::String)
        .field("onSpawnAssignment", pointer(ASSIGNMENT))
        .field("minGroupSize", TestType::Int32)
        .field("maxGroupSize", TestType::Int32)
        .field("maxConcurrentSpawns", TestType::Int32)
        .field("maxSpawns", TestType::Int32)
        .field("minSpawnDelay", TestType::Single)
        .field("maxSpawnDelay", TestType::Single)
        .array("inventoryItems", inline(INVENTORY))
        .field("entityTags", inline("TagList"))
}

fn encounter_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .enumeration(PRESET, ["FixturePreset"])
        .structure(
            TestStruct::new("EncounterIndexEntry")
                .field("encounterPreset", TestType::Enum(PRESET.to_owned()))
                .array("autoSpawns", reference("AutoSpawns")),
        )
        .structure(
            TestStruct::new("EncounterIndex").array("encounters", inline("EncounterIndexEntry")),
        )
}

fn spawn_data(selector: TestInstance) -> TestValue {
    class(SPAWN_DATA)
        .value("class", selector.into_value())
        .value("identity", TestValue::Null)
        .value("initialActivity", TestValue::text("FixtureActivity"))
        .value("onSpawnAssignment", assignment())
        .value("minGroupSize", TestValue::Int32(-1))
        .value("maxGroupSize", TestValue::Int32(2))
        .value("maxConcurrentSpawns", TestValue::Int32(0))
        .value("maxSpawns", TestValue::Int32(4))
        .value("minSpawnDelay", TestValue::Single(-0.0))
        .value("maxSpawnDelay", TestValue::Single(0.125))
        .value(
            "inventoryItems",
            array([class(INVENTORY)
                .value("entityClass", TestValue::Reference(MISSING))
                .value("quantity", TestValue::Int32(0))
                .value("amountInventoriesToFill", TestValue::Int32(2))
                .value("markAsMissionItem", TestValue::Bool(true))
                .into_value()]),
        )
        .value("entityTags", tag_list([]))
        .into_value()
}

fn assignment() -> TestValue {
    class("ScheduleAreaAssignment")
        .value(
            "areaPositiveTags",
            tag_list([TestValue::Reference(MISSING), TestValue::Null]),
        )
        .value("areaNegativeTags", tag_list([]))
        .into_value()
}

fn tag_list<const N: usize>(tags: [TestValue; N]) -> TestValue {
    class("TagList").value("tags", array(tags)).into_value()
}

fn archetype_class() -> TestInstance {
    let weight = class("NPCArchetypeWeight")
        .value("weight", TestValue::Single(0.25))
        .value("record", TestValue::Reference(super::ARCHETYPE));
    class("AutoSpawnArchetype").value("archetype", array([weight.into_value()]))
}

fn pool_class() -> TestInstance {
    class("AutoSpawnClassPool").value(
        "classPool",
        array([
            TestValue::Reference(super::POOL),
            TestValue::Reference(super::POOL),
        ]),
    )
}

fn manual(selector: TestInstance) -> TestValue {
    class("AutoSpawnsDataManual")
        .value("autoSpawnData", spawn_data(selector))
        .into_value()
}

fn record(guid: u128, source_type: &str, suffix: &str) -> TestRecord {
    super::record(guid, source_type, suffix)
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn inline(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn class(target: &str) -> TestInstance {
    TestInstance::new(target)
}

fn array<const N: usize>(items: [TestValue; N]) -> TestValue {
    TestValue::Array(items.into())
}
trait IntoValue {
    fn into_value(self) -> TestValue;
}

impl IntoValue for TestInstance {
    fn into_value(self) -> TestValue {
        TestValue::Struct(self)
    }
}
