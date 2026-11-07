// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
fn weak(name: &str) -> TestType {
    TestType::WeakPointer(name.to_owned())
}
fn value(name: &str) -> TestStruct {
    TestStruct::new(format!("MissionPropertyValue_{name}")).parent("BaseMissionPropertyValue")
}

pub(super) fn add(builder: DataForgeBuilder) -> DataForgeBuilder {
    [options(), auto(), investigation(), other()]
        .into_iter()
        .flatten()
        .fold(builder, DataForgeBuilder::structure)
}

fn options() -> Vec<TestStruct> {
    let mut types = vec![
        TestStruct::new("BaseMissionPropertyValueOption")
            .field("textId", TestType::Locale)
            .field("weighting", TestType::Single)
            .field("DEBUG_forceChooseThisOption", TestType::Bool)
            .array("dependentProperties", class("DependentMissionProperty")),
        TestStruct::new("DependentMissionProperty")
            .field("property", weak("MissionProperty"))
            .array("values", weak("BaseMissionPropertyValueOption")),
        TestStruct::new("TagList").array("tags", TestType::Reference),
    ];
    for (kind, number) in [
        ("StringHash", TestType::String),
        ("Integer", TestType::Int32),
        ("Float", TestType::Single),
    ] {
        let target = format!("MissionPropertyValueOption_{kind}");
        types.push(value(kind).array("options", class(&target)));
        let mut option = TestStruct::new(&target)
            .parent("BaseMissionPropertyValueOption")
            .field("value", number.clone());
        if kind != "StringHash" {
            option = option.field("variation", number);
        }
        types.push(option);
    }
    types.push(
        value("Tags")
            .field("tags", class("TagList"))
            .field("negativeTags", class("TagList"))
            .array("options", class("MissionPropertyValueOption_Tags")),
    );
    types.push(
        TestStruct::new("MissionPropertyValueOption_Tags")
            .parent("BaseMissionPropertyValueOption")
            .field("tags", class("TagList"))
            .field("negativeTags", class("TagList")),
    );
    types
}

fn auto() -> Vec<TestStruct> {
    vec![
        TestStruct::new("Definition"),
        value("AutoSpawns")
            .field("autoSpawns", TestType::Reference)
            .field("searchOptions", pointer("SpawnClosetSearchOptionsBase")),
        TestStruct::new("SpawnClosetSearchOptionsBase"),
        TestStruct::new("SpawnClosetSearchOptions_EntityDeclaration")
            .parent("SpawnClosetSearchOptionsBase")
            .field("entityDeclaration", TestType::Reference),
        TestStruct::new("SpawnClosetSearchOptions_Tags")
            .parent("SpawnClosetSearchOptionsBase")
            .field("closetPositiveTags", class("TagList"))
            .field("closetNegativeTags", class("TagList")),
        TestStruct::new("SpawnClosetSearchOptions_Unknown").parent("SpawnClosetSearchOptionsBase"),
    ]
}

fn investigation() -> Vec<TestStruct> {
    vec![
        value("Investigation")
            .array("startingJournalEntries", TestType::Reference)
            .array("suspects", class("Investigation_Suspect"))
            .array("locations", class("Investigation_Location")),
        TestStruct::new("Investigation_Suspect")
            .field("name", class("MissionPropertyValue_AIName"))
            .field("initialSuspect", TestType::Bool)
            .field("isGuilty", TestType::Bool),
        TestStruct::new("Investigation_Location")
            .array("clueItems", pointer("Investigation_ClueItem")),
        TestStruct::new("Investigation_ClueItem")
            .field("entityClass", TestType::Reference)
            .field("spawnTags", class("TagList"))
            .array("clues", pointer("Investigation_Clue")),
        TestStruct::new("Investigation_ClueItem_Corpse")
            .parent("Investigation_ClueItem")
            .field("suspectRepresented", weak("Investigation_Suspect")),
        TestStruct::new("Investigation_ClueItem_Datapad")
            .parent("Investigation_ClueItem")
            .field("canvas", TestType::Reference),
        TestStruct::new("Investigation_ClueItem_Unknown").parent("Investigation_ClueItem"),
        TestStruct::new("Investigation_Clue")
            .field("objectiveLongDescription", TestType::Locale)
            .field("objectiveShortDescription", TestType::Locale),
        TestStruct::new("Investigation_Clue_CorpseScanInfo")
            .parent("Investigation_Clue")
            .field("name", class("MissionPropertyValue_AIName"))
            .field("causeOfDeath", TestType::Locale),
        TestStruct::new("Investigation_Clue_JournalEntry")
            .parent("Investigation_Clue")
            .field("journalEntry", TestType::Reference),
        TestStruct::new("Investigation_Clue_Unknown").parent("Investigation_Clue"),
    ]
}

fn other() -> Vec<TestStruct> {
    vec![
        value("AIName")
            .field("randomName", TestType::Bool)
            .field("randomLastName", TestType::Bool)
            .field("randomNickName", TestType::Bool)
            .field("characterGivenName", TestType::Locale)
            .field("characterGivenLastName", TestType::Locale)
            .field("characterGivenNickName", TestType::Locale)
            .field("characterNameData", TestType::Reference)
            .field("chanceOfNickName", TestType::Single),
        value("TimeTrialRace").array("targetSplits", class("TimeTrialSplit")),
        TestStruct::new("TimeTrialSplit").field("targetSplit", TestType::Single),
        value("Organization")
            .array("matchConditions", pointer("DataSetMatchCondition"))
            .field("debugForceChosenOrganization", TestType::Reference),
        TestStruct::new("DataSetMatchCondition"),
        TestStruct::new("DataSetMatchCondition_FactionIdentityDef")
            .parent("DataSetMatchCondition")
            .field("factionIdentity", TestType::Reference),
        TestStruct::new("DataSetMatchCondition_ExcludeNearbyLocationsDef")
            .parent("DataSetMatchCondition")
            .field("locationProperty", weak("MissionProperty"))
            .field("exclusionRangeKm", TestType::Single),
        TestStruct::new("DataSetMatchCondition_ExcludeDistantLocationsDef")
            .parent("DataSetMatchCondition")
            .field("locationProperty", weak("MissionProperty"))
            .field("exclusionRangeKm", TestType::Single),
    ]
}
