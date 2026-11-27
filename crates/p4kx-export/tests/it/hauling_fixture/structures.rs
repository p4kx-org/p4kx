// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The structs of the hauling fixture.

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn content(name: &str) -> TestStruct {
    TestStruct::new(name).parent("HaulingOrderContent")
}

fn order(name: &str) -> TestStruct {
    TestStruct::new(name).parent("HaulingOrder")
}

fn contract_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractGenerator")
            .array("generators", pointer("ContractGeneratorHandler")),
        TestStruct::new("ContractGeneratorHandler"),
        TestStruct::new("ContractGeneratorHandler_Career")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .field("contractParams", class("ContractParamOverrides"))
            .array("contracts", class("CareerContract")),
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
            .field("paramOverrides", class("ContractParamOverrides")),
        TestStruct::new("MissionProperty")
            .field("missionVariableName", TestType::String)
            .field("extendedTextToken", TestType::String)
            .field("value", pointer("MissionPropertyValue")),
        TestStruct::new("MissionPropertyValue"),
        TestStruct::new("MissionPropertyValue_HaulingOrders")
            .parent("MissionPropertyValue")
            .array("haulingOrderContent", pointer("HaulingOrderContent")),
        TestStruct::new("MissionPropertyValue_Boolean")
            .parent("MissionPropertyValue")
            .field("value", TestType::Bool),
    ]
}

fn content_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("HaulingOrderContent"),
        content("HaulingOrderContent_Resource")
            .field("resource", TestType::Reference)
            .field("minSCU", TestType::Int32)
            .field("maxSCU", TestType::Int32)
            .field("maxContainerSize", TestType::Int32),
        content("HaulingOrderContent_EntityClass")
            .field("entityClass", TestType::Reference)
            .field("minAmount", TestType::Int32)
            .field("maxAmount", TestType::Int32),
        content("HaulingOrderContent_EntityClasses")
            .field("haulingEntityClasses", TestType::Reference)
            .field("minAmount", TestType::Int32)
            .field("maxAmount", TestType::Int32),
        content("HaulingOrderContent_MissionItem")
            .field("item", TestType::Reference)
            .field("minAmount", TestType::Int32)
            .field("maxAmount", TestType::Int32),
        content("HaulingOrderContent_Or").array("options", class("HaulingOrder_OrOption_And")),
        TestStruct::new("HaulingOrder_OrOption_And")
            .array("orders", pointer("HaulingOrderContent")),
    ]
}

fn objective_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractTemplate")
            .array("contractProperties", class("MissionProperty"))
            .array("objectiveTokens", class("ObjectiveToken")),
        TestStruct::new("MissionBrokerEntry")
            .field("title", TestType::Locale)
            .array("objectiveTokens", class("ObjectiveToken")),
        TestStruct::new("ObjectiveToken").field("objectiveHandler", pointer("ObjectiveHandler")),
        TestStruct::new("ObjectiveHandler"),
        TestStruct::new("ObjectiveHandler_Hauling")
            .parent("ObjectiveHandler")
            .array("haulingOrders", pointer("HaulingOrder")),
        TestStruct::new("ObjectiveHandler_Other").parent("ObjectiveHandler"),
        TestStruct::new("HaulingOrder"),
        TestStruct::new("ResourceType"),
        TestStruct::new("Placeholder"),
        order("HaulingOrder_Resource")
            .field("resource", TestType::Reference)
            .field("minSCU", TestType::Int32)
            .field("maxSCU", TestType::Int32)
            .field("maxContainerSize", TestType::Int32),
        order("HaulingOrder_EntityClass")
            .field("entityClass", TestType::Reference)
            .field("minAmount", TestType::Int32)
            .field("maxAmount", TestType::Int32),
        order("HaulingOrder_MissionItem")
            .field("item", TestType::Reference)
            .field("minAmount", TestType::Int32),
        order("HaulingOrder_Property").field("haulingOrdersProperty", TestType::Reference),
    ]
}

fn manifest_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("CargoManifest")
            .field("lockCargoGridsOnSpawn", TestType::Bool)
            .array("tags", TestType::String)
            .array("descriptionTags", TestType::Locale)
            .field("cargoFillCapacity", pointer("CargoFillCapacityValue"))
            .array("cargoSpawnRules", pointer("CargoSpawnRule")),
        TestStruct::new("CargoFillCapacityValue"),
        TestStruct::new("CargoFillCapacityValue_Even")
            .parent("CargoFillCapacityValue")
            .field("minCapacityRange", TestType::Single)
            .field("maxCapacityRange", TestType::Single)
            .array("resources", class("CargoResource")),
        TestStruct::new("CargoFillCapacityValue_Random")
            .parent("CargoFillCapacityValue")
            .field("minCapacityRange", TestType::Single)
            .field("maxCapacityRange", TestType::Single)
            .array("resources", class("CargoResource")),
        TestStruct::new("CargoFillCapacityValue_Custom")
            .parent("CargoFillCapacityValue")
            .array("resources", class("CargoResourceAllocation")),
        TestStruct::new("CargoResource")
            .field("probability", TestType::Single)
            .field("resource", TestType::Reference),
        TestStruct::new("CargoResourceAllocation")
            .field("minResourceAllocationPercentage", TestType::Single)
            .field("maxResourceAllocationPercentage", TestType::Single)
            .field("resources", class("CargoResource")),
        TestStruct::new("CargoSpawnRule")
            .field("probability", TestType::Single)
            .field("quantityAlgorithmType", pointer("CargoManifestQuantityDef")),
        TestStruct::new("CargoSpawnEntityDef")
            .parent("CargoSpawnRule")
            .array("entities", TestType::Reference),
        TestStruct::new("CargoSpawnResourceDef")
            .parent("CargoSpawnRule")
            .array("resources", TestType::Reference),
        TestStruct::new("CargoSpawnFutureDef")
            .parent("CargoSpawnRule")
            .field("futureQuota", TestType::UInt64),
        TestStruct::new("CargoManifestQuantityDef"),
        TestStruct::new("CargoManifestFixedQuantityDef")
            .parent("CargoManifestQuantityDef")
            .field("quantity", TestType::Int32),
        TestStruct::new("CargoManifestProportionQuantityDef")
            .parent("CargoManifestQuantityDef")
            .field("proportion", TestType::Single),
    ]
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        contract_structs(),
        content_structs(),
        objective_structs(),
        manifest_structs(),
    ]
    .into_iter()
    .flatten()
    .fold(builder, DataForgeBuilder::structure)
}
