// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The struct definitions of the synthetic Archive of the contract facts.

use p4kx_testkit::{TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

/// Returns every struct of the fixture.
pub(crate) fn all() -> Vec<TestStruct> {
    [
        handler_structs(),
        contract_structs(),
        prerequisite_structs(),
        result_structs(),
        template_structs(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn handler_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractGenerator").array("generators", pointer("ContractHandler")),
        TestStruct::new("ContractHandler"),
        TestStruct::new("ContractHandler_Bare")
            .parent("ContractHandler")
            .field("debugName", TestType::String)
            .array("contracts", class("BareContract")),
        TestStruct::new("BareContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String),
        TestStruct::new("ContractHandler_Career")
            .parent("ContractHandler")
            .field("debugName", TestType::String)
            .field("defaultAvailability", class("ContractAvailability"))
            .field("contractParams", class("ContractParamOverrides"))
            .array("contracts", class("CareerContract"))
            .array("introContracts", class("CareerContract")),
        TestStruct::new("ContractAvailability")
            .field("onceOnly", TestType::Bool)
            .field("maxPlayersPerInstance", TestType::Int32)
            .field("availableInPrison", TestType::Bool)
            .field("canReacceptAfterAbandoning", TestType::Bool)
            .field("canReacceptAfterFailing", TestType::Bool)
            .field("hideInMobiGlas", TestType::Bool)
            .field("hasPersonalCooldown", TestType::Bool)
            .field("personalCooldownTime", TestType::Single)
            .field("abandonedCooldownTime", TestType::Single)
            .array("prerequisites", pointer("ContractPrerequisite")),
    ]
}

fn prerequisite_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractPrerequisite"),
        TestStruct::new("ContractPrerequisite_CompletedContractTags")
            .parent("ContractPrerequisite")
            .field("requiredCompletedContractTags", class("TagList")),
        TestStruct::new("TagList").array("tags", TestType::Reference),
    ]
}

fn contract_structs() -> Vec<TestStruct> {
    let params = || {
        [
            ("boolParamOverrides", "ContractBoolParam"),
            ("intParamOverrides", "ContractIntParam"),
        ]
    };
    let mut overrides = TestStruct::new("ContractParamOverrides");
    let mut sub = TestStruct::new("SubContract")
        .field("id", TestType::Guid)
        .field("debugName", TestType::String)
        .field("notForRelease", TestType::Bool)
        .field("workInProgress", TestType::Bool)
        .array("additionalPrerequisites", pointer("ContractPrerequisite"));
    for (field, kind) in params() {
        overrides = overrides.array(field, class(kind));
        sub = sub.array(field, class(kind));
    }
    vec![
        overrides,
        TestStruct::new("ContractBoolParam")
            .field("param", TestType::String)
            .field("value", TestType::Bool),
        TestStruct::new("ContractIntParam")
            .field("param", TestType::String)
            .field("value", TestType::Int32),
        TestStruct::new("CareerContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("notForRelease", TestType::Bool)
            .field("workInProgress", TestType::Bool)
            .field("template", TestType::Reference)
            .field("paramOverrides", class("ContractParamOverrides"))
            .array("subContracts", class("SubContract"))
            .array("additionalPrerequisites", pointer("ContractPrerequisite"))
            .field("contractResults", class("ContractResultBlock")),
        sub,
    ]
}

fn result_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractResultBlock")
            .field("timeToComplete", TestType::Single)
            .field("contractBuyInAmount", TestType::Int32)
            .array("contractResults", pointer("ContractResult")),
        TestStruct::new("ContractResult"),
        TestStruct::new("ContractResult_CompletionTags")
            .parent("ContractResult")
            .array("missionResults", class("Bool"))
            .array("completionTags", class("ContractResult_CompletionTag")),
        TestStruct::new("ContractResult_CompletionTag")
            .field("count", TestType::Int32)
            .field("tag", TestType::Reference),
        TestStruct::new("Bool").field("value", TestType::Bool),
    ]
}

fn template_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractTemplate")
            .field("contractDisplayInfo", class("ContractDisplayInfo"))
            .field("contractClass", pointer("ContractClass")),
        TestStruct::new("ContractDisplayInfo").field("illegal", TestType::Bool),
        TestStruct::new("ContractClass"),
        TestStruct::new("ContractClass_Contract")
            .parent("ContractClass")
            .array("additionalParams", pointer("ContractAdditionalParams")),
        TestStruct::new("ContractAdditionalParams"),
        TestStruct::new("ActiveContractSettings")
            .parent("ContractAdditionalParams")
            .field("canBeShared", TestType::Bool),
        TestStruct::new("Tag").field("tagName", TestType::String),
    ]
}
