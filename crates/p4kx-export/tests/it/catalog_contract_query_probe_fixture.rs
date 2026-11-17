// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const CONSUMER: u128 = 0xa200_0000_0000_4000_8000_0000_0000_0001;
pub(super) const INTRO: u128 = 0xa300_0000_0000_4000_8000_0000_0000_0001;
const OWNER: u128 = 0xa100_0000_0000_4000_8000_0000_0000_0001;
const TEMPLATE: u128 = 0xa400_0000_0000_4000_8000_0000_0000_0001;
const BROKER: u128 = 0xa500_0000_0000_4000_8000_0000_0000_0001;
const TAG: u128 = 0xa600_0000_0000_4000_8000_0000_0000_0001;

pub(super) fn database() -> Vec<u8> {
    schema()
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(generator())
        .record(TestRecord::new(
            TEMPLATE,
            "ContractTemplate",
            "ContractTemplate.Required",
            "libs/foundry/records/contracts/contracttemplates/required.xml",
        ))
        .record(TestRecord::new(
            BROKER,
            "MissionBrokerEntry",
            "MissionBrokerEntry.Required",
            "libs/foundry/records/missionbroker/required.xml",
        ))
        .record(TestRecord::new(
            TAG,
            "Tag",
            "Tag.Intro",
            "libs/foundry/records/tags/intro.xml",
        ))
        .build()
}

fn schema() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractGenerator").array("generators", pointer("Handler")),
        TestStruct::new("Handler")
            .array("contracts", pointer("Contract"))
            .array("introContracts", pointer("Contract")),
        TestStruct::new("Contract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("template", TestType::Reference)
            .field("missionBrokerEntry", TestType::Reference)
            .field("paramOverrides", class("Params"))
            .field("contractResults", class("ResultBlock"))
            .array("additionalPrerequisites", pointer("Prerequisite")),
        TestStruct::new("Params").array("stringParamOverrides", class("StringParam")),
        TestStruct::new("StringParam")
            .field("param", TestType::String)
            .field("value", TestType::Locale),
        TestStruct::new("Prerequisite"),
        TestStruct::new("ContractPrerequisite_CompletedContractTags")
            .parent("Prerequisite")
            .field("requiredCompletedContractTags", class("TagList")),
        TestStruct::new("TagList").array("tags", TestType::Reference),
        TestStruct::new("ResultBlock").array("contractResults", pointer("Result")),
        TestStruct::new("Result"),
        TestStruct::new("ContractResult_CompletionTags")
            .parent("Result")
            .array("missionResults", TestType::Bool)
            .array("completionTags", class("CompletionTag")),
        TestStruct::new("CompletionTag").field("tag", TestType::Reference),
        TestStruct::new("ContractTemplate"),
        TestStruct::new("MissionBrokerEntry"),
        TestStruct::new("Tag"),
    ]
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
const fn array(values: Vec<TestValue>) -> TestValue {
    TestValue::Array(values)
}
const fn structure(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}

fn contract(guid: u128, title: &str) -> TestInstance {
    TestInstance::new("Contract")
        .value("id", TestValue::Guid(guid))
        .value("debugName", TestValue::text("Canonical"))
        .value(
            "paramOverrides",
            structure(TestInstance::new("Params").value(
                "stringParamOverrides",
                array(vec![structure(
                        TestInstance::new("StringParam")
                            .value("param", TestValue::text("Title"))
                            .value("value", TestValue::text(title)),
                    )]),
            )),
        )
}

fn prerequisite() -> TestValue {
    structure(
        TestInstance::new("ContractPrerequisite_CompletedContractTags").value(
            "requiredCompletedContractTags",
            structure(
                TestInstance::new("TagList").value("tags", array(vec![TestValue::Reference(TAG)])),
            ),
        ),
    )
}

fn grant() -> TestValue {
    structure(TestInstance::new("ResultBlock").value(
        "contractResults",
        array(vec![structure(
                TestInstance::new("ContractResult_CompletionTags")
                    .value("missionResults", array(vec![TestValue::Bool(true)]))
                    .value(
                        "completionTags",
                        array(vec![structure(
                            TestInstance::new("CompletionTag")
                                .value("tag", TestValue::Reference(TAG)),
                        )]),
                    ),
            )]),
    ))
}

fn generator() -> TestRecord {
    let consumer =
        contract(CONSUMER, "@shared").value("additionalPrerequisites", array(vec![prerequisite()]));
    let intro = contract(INTRO, "@shared").value("contractResults", grant());
    let late = contract(INTRO + 1, "@later_name")
        .value("template", TestValue::Reference(TEMPLATE))
        .value("missionBrokerEntry", TestValue::Reference(BROKER));
    let handler = TestInstance::new("Handler")
        .value("contracts", array(vec![structure(consumer)]))
        .value(
            "introContracts",
            array(vec![structure(intro), structure(late)]),
        );
    TestRecord::new(
        OWNER,
        "ContractGenerator",
        "ContractGenerator.Transactions",
        "libs/foundry/records/contracts/contractgenerator/transactions.xml",
    )
    .value("generators", array(vec![structure(handler)]))
}
