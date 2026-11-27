// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::mission_property_sets_fixture::{self as sets, parameters};
use crate::mission_values_fixture::{self as values, Owner};
use crate::scoped_fixture::property;

pub(super) const SELECTED_CONTRACT: u128 = 0x8800_0000_0000_4000_8000_0000_0000_00f0;

pub(super) const TEXTS: [(&str, &str); 7] = [
    ("given", "Given name"),
    ("family", "Family name"),
    ("nick", "Nickname"),
    ("long", "Long clue"),
    ("short", "Short clue"),
    ("cause", "Cause of death"),
    ("option_name", "Option name"),
];

pub(super) fn database() -> DataForgeBuilder {
    let properties = properties();
    let builder = values::database(properties.clone(), Owner::Generator);
    npc_schema()
        .into_iter()
        .fold(builder, DataForgeBuilder::structure)
        .record(selected(properties))
}

fn selected(properties: Vec<TestValue>) -> TestRecord {
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(SELECTED_CONTRACT))
        .value("debugName", TestValue::text("Selected"))
        .value("paramOverrides", parameters(properties.clone()));
    sets::owner().value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("ContractGeneratorHandler_Career")
                .value("debugName", TestValue::text("ScopedHandler"))
                .value("contractParams", parameters(properties))
                .value(
                    "contracts",
                    TestValue::Array(vec![TestValue::Struct(contract)]),
                ),
        )]),
    )
}

fn properties() -> Vec<TestValue> {
    let investigation = values::localized_probe(["@long", "@short", "@cause"]).value(
        "suspects",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("Investigation_Suspect")
                .value("name", TestValue::Struct(name()))
                .value("initialSuspect", TestValue::Bool(true)),
        )]),
    );
    let option = TestInstance::new("MissionPropertyValueOption_Integer")
        .value("textId", TestValue::text("@option_name"))
        .value("value", TestValue::Int32(7));
    vec![
        property("Name", name()),
        property("Clues", investigation),
        property(
            "Number",
            TestInstance::new("MissionPropertyValue_Integer")
                .value("options", TestValue::Array(vec![TestValue::Struct(option)])),
        ),
        property("Crew", npc()),
    ]
}

fn name() -> TestInstance {
    TestInstance::new("MissionPropertyValue_AIName")
        .value("randomName", TestValue::Bool(false))
        .value("randomLastName", TestValue::Bool(false))
        .value("randomNickName", TestValue::Bool(false))
        .value("characterGivenName", TestValue::text("@given"))
        .value("characterGivenLastName", TestValue::text("@family"))
        .value("characterGivenNickName", TestValue::text("@nick"))
        .value("characterNameData", TestValue::Reference(values::TARGET))
}

fn npc() -> TestInstance {
    let option = TestInstance::new("SpawnDescription_NPCOption")
        .value("embeddedName", TestValue::Struct(name()));
    let group = TestInstance::new("SpawnDescription_NPC_Group")
        .value("Name", TestValue::text("Crew"))
        .value("options", TestValue::Array(vec![TestValue::Struct(option)]));
    TestInstance::new("MissionPropertyValue_NPCSpawnDescriptions").value(
        "spawnDescriptions",
        TestValue::Array(vec![TestValue::Struct(group)]),
    )
}

fn npc_schema() -> [TestStruct; 4] {
    [
        TestStruct::new("SpawnDescription"),
        TestStruct::new("MissionPropertyValue_NPCSpawnDescriptions")
            .parent("BaseMissionPropertyValue")
            .array(
                "spawnDescriptions",
                TestType::StrongPointer("SpawnDescription".to_owned()),
            ),
        TestStruct::new("SpawnDescription_NPC_Group")
            .parent("SpawnDescription")
            .field("Name", TestType::String)
            .array(
                "options",
                TestType::Class("SpawnDescription_NPCOption".to_owned()),
            ),
        TestStruct::new("SpawnDescription_NPCOption")
            .field("priority", TestType::Int32)
            .field("includeLocationAISpawnTags", TestType::Bool)
            .field("weight", TestType::Single)
            .field("identifierTags", TestType::Class("TagList".to_owned()))
            .field(
                "embeddedName",
                TestType::StrongPointer("MissionPropertyValue_AIName".to_owned()),
            ),
    ]
}
