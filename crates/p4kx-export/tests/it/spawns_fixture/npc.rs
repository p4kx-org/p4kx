// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! NPC and entity spawn descriptions and organisation values of the fixture's properties.

use p4kx_testkit::{TestInstance, TestStruct, TestType, TestValue};

use super::template_parts::object;
use super::{OWNER, TAG_HIJACKED, TAG_KILL, TAG_SHIP, array, property, reference, text};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn value(name: &str) -> TestStruct {
    TestStruct::new(name).parent("MissionPropertyValue")
}

/// The structs of the NPC, entity and organisation values.
pub(super) fn structs() -> Vec<TestStruct> {
    let mut structs = vec![
        value("MissionPropertyValue_NPCSpawnDescriptions")
            .array("spawnDescriptions", pointer("SpawnDescription")),
        TestStruct::new("SpawnDescription_NPC_Group")
            .parent("SpawnDescription")
            .field("Name", TestType::String)
            .array("options", class("SpawnDescription_NPCOption")),
        TestStruct::new("SpawnDescription_NPCOption")
            .field("priority", TestType::Int32)
            .field("includeLocationAISpawnTags", TestType::Bool)
            .field("weight", TestType::Single)
            .field("autoSpawnSettings", class("AutoSpawnSettings"))
            .field("identifierTags", class("TagList"))
            .field(
                "nameProperty",
                TestType::WeakPointer("MissionProperty".into()),
            )
            .field("embeddedName", pointer("MissionPropertyValue_AIName")),
        TestStruct::new("SpawnSettingsInventoryItem")
            .field("entityClass", TestType::Reference)
            .field("quantity", TestType::Int32)
            .field("amountInventoriesToFill", TestType::Int32)
            .field("markAsMissionItem", TestType::Bool),
        value("MissionPropertyValue_AIName")
            .field("randomName", TestType::Bool)
            .field("randomLastName", TestType::Bool)
            .field("randomNickName", TestType::Bool)
            .field("characterGivenName", TestType::String)
            .field("characterGivenLastName", TestType::String)
            .field("characterGivenNickName", TestType::String)
            .field("chanceOfNickName", TestType::Single)
            .field("characterNameData", TestType::Reference),
        value("MissionPropertyValue_EntitySpawnDescriptions")
            .array("spawnDescriptions", pointer("SpawnDescription")),
        TestStruct::new("SpawnDescription_EntityGroup")
            .parent("SpawnDescription")
            .field("Name", TestType::String)
            .array("entities", class("SpawnDescription_EntityOptions")),
        TestStruct::new("SpawnDescription_EntityOptions")
            .array("options", class("SpawnDescription_Entity")),
        TestStruct::new("SpawnDescription_Entity")
            .field("amount", TestType::Int32)
            .field("weight", TestType::Single)
            .field("tags", class("TagList"))
            .field("negativeTags", class("TagList"))
            .field("markupTags", class("TagList"))
            .field("entityTags", class("TagList")),
        value("MissionPropertyValue_Organization")
            .array("matchConditions", pointer("DataSetMatchCondition"))
            .field("debugForceChosenOrganization", TestType::Reference),
        TestStruct::new("DataSetMatchCondition_SpecificOrganizationsDef")
            .parent("DataSetMatchCondition")
            .array("organizations", TestType::Reference),
        TestStruct::new("DataSetMatchCondition_Mystery").parent("DataSetMatchCondition"),
        value("MissionPropertyValue_MissionItem"),
    ];
    structs.push(auto_spawn_struct());
    structs
}

fn auto_spawn_struct() -> TestStruct {
    let tag_lists = [
        "positiveCharacterTags",
        "negativeCharacterTags",
        "closetPositiveTags",
        "closetNegativeTags",
        "roomPositiveTags",
        "roomNegativeTags",
        "defendAreaPositiveTags",
        "defendAreaNegativeTags",
        "scheduleAreaPositiveTags",
        "scheduleAreaNegativeTags",
        "entityTags",
    ];
    let numbers = [
        "minGroupSize",
        "maxGroupSize",
        "maxConcurrentSpawns",
        "maxSpawns",
        "minSpawnDelay",
        "maxSpawnDelay",
    ];
    let with_numbers = numbers.into_iter().fold(
        TestStruct::new("AutoSpawnSettings")
            .field("name", TestType::String)
            .field("initialActivity", TestType::String)
            .field("excludeShipCrew", TestType::Bool)
            .field("excludeSpawnGender", TestType::String)
            .field("missionAlliedMarker", TestType::Bool)
            .field("isCritical", TestType::Bool)
            .field("factionOverride", TestType::Reference)
            .array("inventoryItems", class("SpawnSettingsInventoryItem")),
        |built, name| built.field(name, TestType::Int32),
    );
    tag_lists.into_iter().fold(with_numbers, |built, name| {
        built.field(name, class("TagList"))
    })
}

fn tags(tags: &[u128]) -> TestValue {
    object(
        TestInstance::new("TagList")
            .value("tags", array(tags.iter().copied().map(reference).collect())),
    )
}

fn auto_spawn() -> TestValue {
    let mut settings = TestInstance::new("AutoSpawnSettings")
        .value("name", text("Squad (Grenadier x1)"))
        .value("initialActivity", text("Sentry"))
        .value("excludeShipCrew", TestValue::Bool(true))
        .value("excludeSpawnGender", text("Female"))
        .value("missionAlliedMarker", TestValue::Bool(false))
        .value("isCritical", TestValue::Bool(true))
        .value("factionOverride", reference(OWNER))
        .value(
            "inventoryItems",
            array(vec![object(
                TestInstance::new("SpawnSettingsInventoryItem")
                    .value("entityClass", reference(OWNER))
                    .value("quantity", TestValue::Int32(3))
                    .value("amountInventoriesToFill", TestValue::Int32(1))
                    .value("markAsMissionItem", TestValue::Bool(true)),
            )]),
        );
    for (name, number) in [
        ("minGroupSize", -1),
        ("maxGroupSize", 4),
        ("maxConcurrentSpawns", 4),
        ("maxSpawns", 6),
        ("minSpawnDelay", 0),
        ("maxSpawnDelay", 10),
    ] {
        settings = settings.value(name, TestValue::Int32(number));
    }
    for (name, list) in [
        ("positiveCharacterTags", vec![TAG_SHIP]),
        ("negativeCharacterTags", vec![TAG_KILL]),
        ("closetPositiveTags", vec![TAG_HIJACKED]),
        ("closetNegativeTags", vec![]),
        ("roomPositiveTags", vec![]),
        ("roomNegativeTags", vec![]),
        ("defendAreaPositiveTags", vec![TAG_KILL]),
        ("defendAreaNegativeTags", vec![]),
        ("scheduleAreaPositiveTags", vec![]),
        ("scheduleAreaNegativeTags", vec![TAG_SHIP]),
        ("entityTags", vec![TAG_HIJACKED]),
    ] {
        settings = settings.value(name, tags(&list));
    }
    object(settings)
}

fn ai_name() -> TestValue {
    object(
        TestInstance::new("MissionPropertyValue_AIName")
            .value("randomName", TestValue::Bool(true))
            .value("randomLastName", TestValue::Bool(false))
            .value("randomNickName", TestValue::Bool(true))
            .value("characterGivenName", text("@place_name"))
            .value("characterGivenLastName", text("@LOC_UNINITIALIZED"))
            .value("characterGivenNickName", text("@LOC_EMPTY"))
            .value("chanceOfNickName", TestValue::Single(0.25))
            .value("characterNameData", reference(OWNER)),
    )
}

/// What an NPC option of the fixture carries besides its settings.
#[derive(Clone, Copy)]
pub(crate) enum NpcKind {
    /// An embedded name.
    Full,
    /// The `nameProperty` pointing to the property named by the pointer.
    Named(&'static str),
}

fn npc_option(kind: NpcKind) -> TestValue {
    let option = TestInstance::new("SpawnDescription_NPCOption")
        .value("priority", TestValue::Int32(2))
        .value("includeLocationAISpawnTags", TestValue::Bool(true))
        .value("weight", TestValue::Single(0.5))
        .value("autoSpawnSettings", auto_spawn())
        .value("identifierTags", tags(&[TAG_KILL]));
    object(match kind {
        NpcKind::Full => option.value("embeddedName", ai_name()),
        NpcKind::Named(pointer) => option.value("nameProperty", TestValue::pointer(pointer)),
    })
}

/// An NPC spawn property with the group `group` of one option; `None` has no group at all.
pub(crate) fn npcs(variable: &str, token: &str, group: Option<(&str, NpcKind)>) -> TestValue {
    let groups = group.map_or_else(Vec::new, |(name, kind)| {
        vec![object(
            TestInstance::new("SpawnDescription_NPC_Group")
                .value("Name", text(name))
                .value("options", array(vec![npc_option(kind)])),
        )]
    });
    property(
        variable,
        token,
        TestInstance::new("MissionPropertyValue_NPCSpawnDescriptions")
            .value("spawnDescriptions", array(groups)),
    )
}

/// An entity spawn property with the group `group` of one slot of one option; `None` has no group.
pub(crate) fn entities(variable: &str, token: &str, group: Option<&str>) -> TestValue {
    let groups = group.map_or_else(Vec::new, |name| {
        let option = TestInstance::new("SpawnDescription_Entity")
            .value("amount", TestValue::Int32(10))
            .value("weight", TestValue::Single(0.15))
            .value("tags", tags(&[TAG_SHIP]))
            .value("negativeTags", tags(&[]))
            .value("markupTags", tags(&[TAG_KILL]))
            .value("entityTags", tags(&[]));
        vec![object(
            TestInstance::new("SpawnDescription_EntityGroup")
                .value("Name", text(name))
                .value(
                    "entities",
                    array(vec![object(
                        TestInstance::new("SpawnDescription_EntityOptions")
                            .value("options", array(vec![object(option)])),
                    )]),
                ),
        )]
    });
    property(
        variable,
        token,
        TestInstance::new("MissionPropertyValue_EntitySpawnDescriptions")
            .value("spawnDescriptions", array(groups)),
    )
}

/// An organisation property of one organisation condition, or of a condition of a struct the
/// catalog does not know.
pub(crate) fn organization(variable: &str, known: bool) -> TestValue {
    let condition = if known {
        TestInstance::new("DataSetMatchCondition_SpecificOrganizationsDef")
            .value("organizations", array(vec![reference(OWNER)]))
    } else {
        TestInstance::new("DataSetMatchCondition_Mystery")
    };
    property(
        variable,
        "",
        TestInstance::new("MissionPropertyValue_Organization")
            .value("matchConditions", array(vec![object(condition)]))
            .value("debugForceChosenOrganization", reference(OWNER)),
    )
}

/// A property whose value is a struct the catalog does not know.
pub(crate) fn mission_item(variable: &str) -> TestValue {
    property(
        variable,
        "",
        TestInstance::new("MissionPropertyValue_MissionItem"),
    )
}
