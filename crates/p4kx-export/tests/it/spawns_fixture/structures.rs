// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn value(name: &str) -> TestStruct {
    TestStruct::new(name).parent("MissionPropertyValue")
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        contracts(),
        templates(),
        super::template_structures::structs(),
        super::template_values::structs(),
        super::broker_structures::structs(),
        super::npc::structs(),
        properties(),
        spawns(),
        locations(),
    ]
    .into_iter()
    .flatten()
    .fold(builder, DataForgeBuilder::structure)
}

fn contracts() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractGenerator")
            .array("generators", pointer("ContractGeneratorHandler")),
        TestStruct::new("ContractGeneratorHandler"),
        TestStruct::new("ContractGeneratorHandler_Career")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .field("defaultAvailability", class("ContractAvailability"))
            .field("contractParams", class("ContractParamOverrides"))
            .array("contracts", class("CareerContract")),
        TestStruct::new("ContractAvailability")
            .array("prerequisites", pointer("ContractPrerequisite")),
        TestStruct::new("ContractPrerequisite"),
        TestStruct::new("ContractPrerequisite_LocationProperty")
            .parent("ContractPrerequisite")
            .field("locationLevelType", TestType::String)
            .field("propertyExtendedTextToken", TestType::String)
            .field("propertyVariableName", TestType::String),
        TestStruct::new("ContractParamOverrides")
            .array("propertyOverrides", class("MissionProperty")),
        TestStruct::new("CareerContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("template", TestType::Reference)
            .field("missionBrokerEntry", TestType::Reference)
            .field("paramOverrides", class("ContractParamOverrides"))
            .array("subContracts", class("SubContract")),
        TestStruct::new("SubContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("additionalPrerequisites", pointer("ContractPrerequisite"))
            .field("paramOverrides", class("ContractParamOverrides")),
        TestStruct::new("ContractTemplate")
            .field("owner", TestType::Reference)
            .field("partialRewardPayout", pointer("PartialContractRewards"))
            .array("modifiers", pointer("MissionModifier"))
            .field("missionFlow", class("MissionFlow"))
            .field(
                "contractStartCommsNotification",
                pointer("ContractCommsNotification"),
            )
            .array(
                "contractEndCommsNotifications",
                class("ContractEndCommsNotification"),
            )
            .field("notForRelease", TestType::Bool)
            .field("contractDisplayInfo", class("ContractDisplayInfo"))
            .field("contractClass", pointer("ContractClass"))
            .array("contractProperties", class("MissionProperty"))
            .array("objectiveTokens", class("ObjectiveToken")),
    ]
}

/// The structs of the contract template Records beyond their properties.
fn templates() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractDisplayInfo")
            .field("illegal", TestType::Bool)
            .field("showLifeTimeInMobiGlas", TestType::Bool)
            .field("preShowObjectives", TestType::Bool)
            .array("displayString", TestType::Locale)
            .field("type", TestType::Reference),
        TestStruct::new("ContractClass"),
        TestStruct::new("ContractClass_Contract")
            .parent("ContractClass")
            .field("additionalParams", class("ActiveContractSettings"))
            .field("autoFinishSettings", class("ContractAutoFinishSettings")),
        TestStruct::new("ActiveContractSettings")
            .field("canBeShared", TestType::Bool)
            .field("hasCompleteButton", TestType::Bool)
            .field("handlesAbandonRequest", TestType::Bool)
            .field("displayAlliedMarkers", TestType::Bool)
            .field("onlyOwnerCanComplete", TestType::Bool),
        TestStruct::new("ContractAutoFinishSettings")
            .field("failIfSentToPrison", TestType::Bool)
            .field("failIfBecameCriminal", TestType::Bool)
            .field("failIfLeavePrison", TestType::Bool)
            .field("contractDeadline", class("MissionDeadline")),
        TestStruct::new("MissionDeadline")
            .field("missionCompletionTime", TestType::Int32)
            .field("missionAutoEnd", TestType::Bool)
            .field("missionResultAfterTimerEnd", TestType::String)
            .field("remainingTimeToShowTimer", TestType::Int32),
        TestStruct::new("HaulingOrder_Resource")
            .parent("HaulingOrder")
            .field("resource", TestType::Reference)
            .field("minSCU", TestType::Int32)
            .field("maxSCU", TestType::Int32),
        value("MissionPropertyValue_Boolean").field("value", TestType::Bool),
        value("MissionPropertyValue_HaulingOrders")
            .array("haulingOrderContent", pointer("HaulingOrder")),
    ]
}

fn properties() -> Vec<TestStruct> {
    vec![
        TestStruct::new("MissionProperty")
            .field("missionVariableName", TestType::String)
            .field("extendedTextToken", TestType::String)
            .field("value", pointer("MissionPropertyValue")),
        TestStruct::new("MissionPropertyValue"),
        TestStruct::new("TagList").array("tags", TestType::Reference),
        TestStruct::new("TagSearchTerm")
            .array("positiveTags", TestType::Reference)
            .array("negativeTags", TestType::Reference),
        TestStruct::new("Tag")
            .field("tagName", TestType::String)
            .field("legacyGUID", TestType::UInt32)
            .array("children", TestType::ReferenceTo("Tag".into())),
        TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".into())),
    ]
}

fn spawns() -> Vec<TestStruct> {
    vec![
        value("MissionPropertyValue_ShipSpawnDescriptions")
            .field("allowedForMissionRestrictedDeliveries", TestType::Bool)
            .array("spawnDescriptions", pointer("SpawnDescription")),
        TestStruct::new("SpawnDescription"),
        TestStruct::new("SpawnDescription_ShipGroup")
            .parent("SpawnDescription")
            .field("Name", TestType::String)
            .array("ships", class("SpawnDescription_ShipOptions")),
        TestStruct::new("SpawnDescription_ShipOptions")
            .array("options", class("SpawnDescription_Ship")),
        TestStruct::new("SpawnDescription_Ship")
            .field("concurrentAmount", TestType::Int32)
            .field("includeLocationAISpawnTags", TestType::Bool)
            .field("weight", TestType::Int32)
            .field("tags", class("TagList"))
            .field("negativeTags", class("TagList"))
            .field("markupTags", class("TagList"))
            .field("entityTags", class("TagList")),
    ]
}

fn locations() -> Vec<TestStruct> {
    vec![
        value("MissionPropertyValue_Location")
            .field("logErrorOnSearchFail", TestType::Bool)
            .array("matchConditions", pointer("DataSetMatchCondition"))
            .array("resourceTags", TestType::Reference),
        value("MissionPropertyValue_Locations")
            .field("failIfMinAmountNotFound", TestType::Bool)
            .field("logErrorOnSearchFail", TestType::Bool)
            .field("maxLocationsToFind", TestType::Int32)
            .field("minLocationsToFind", TestType::Int32)
            .array("matchConditions", pointer("DataSetMatchCondition"))
            .array("resourceTags", TestType::Reference),
        TestStruct::new("DataSetMatchCondition"),
        TestStruct::new("DataSetMatchCondition_TagSearch")
            .parent("DataSetMatchCondition")
            .field("tagType", TestType::String)
            .array("tagSearch", class("TagSearchTerm")),
        TestStruct::new("MissionLocationTemplate")
            .field("locationData", class("MissionLocationData")),
        TestStruct::new("MissionLocationData")
            .field("disabled", TestType::Bool)
            .field("isSecurityNetworkHost", TestType::Bool)
            .field("generalTags", class("MissionLocationTags"))
            .field("producesTags", class("TagSearchTerm"))
            .field("consumesTags", class("TagSearchTerm"))
            .field("aiSpawnTags", class("TagList"))
            .field("stringVariants", class("MissionStringVariants")),
        TestStruct::new("MissionLocationTags").array("tags", TestType::Reference),
        TestStruct::new("MissionStringVariants").array("variants", class("MissionStringVariant")),
        TestStruct::new("MissionStringVariant")
            .field("tag", TestType::Reference)
            .field("string", TestType::Locale),
    ]
}
