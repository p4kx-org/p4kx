// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const OWNER: u128 = 0xb100_0000_0000_4000_8000_0000_0000_0001;
const CONTRACT: u128 = 0xb200_0000_0000_4000_8000_0000_0000_0001;
const TARGET: u128 = 0xb300_0000_0000_4000_8000_0000_0000_0001;
const REWARDS: &str = "libs/foundry/records/reputation/rewards/missionrewards_reputation/";

pub(super) fn cross_family() -> Vec<u8> {
    database()
        .record(generator(
            contract().value("template", TestValue::Reference(TARGET)),
        ))
        .record(TestRecord::new(
            TARGET,
            "ContractTemplate",
            "ContractTemplate.Used",
            "libs/foundry/records/contracts/contracttemplates/used.xml",
        ))
        .record(
            TestRecord::new(
                TARGET,
                "StarMapObject",
                "StarMapObject.OtherFamily",
                "libs/foundry/records/starmap/pu/other_family.xml",
            )
            .with_instance_index(u16::MAX),
        )
        .build()
}

pub(super) fn rewards() -> Vec<u8> {
    let amount = TestInstance::new("Amount").value("reward", TestValue::Reference(TARGET));
    let result = TestInstance::new("ContractResult_LegacyReputation")
        .value(
            "missionResults",
            TestValue::Array(vec![TestValue::Bool(true)]),
        )
        .value(
            "contractResultReputationAmounts",
            TestValue::Array(vec![TestValue::Struct(amount)]),
        );
    let parent = contract().value(
        "contractResults",
        TestValue::Struct(TestInstance::new("ResultBlock").value(
            "contractResults",
            TestValue::Array(vec![TestValue::Struct(result)]),
        )),
    );
    database()
        .record(generator(parent))
        .record(reward(TARGET, "First", 5))
        .record(reward(TARGET, "LastUsable", 9))
        .record(TestRecord::new(
            TARGET,
            "Broken",
            "Broken.Later",
            format!("{REWARDS}later.xml"),
        ))
        .record(TestRecord::new(
            TARGET + 1,
            "Broken",
            "Broken.Unused",
            format!("{REWARDS}unused.xml"),
        ))
        .build()
}

fn database() -> DataForgeBuilder {
    [
        TestStruct::new("ContractGenerator").array("generators", pointer("Handler")),
        TestStruct::new("Handler").array("contracts", pointer("Contract")),
        TestStruct::new("Contract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("template", TestType::Reference)
            .field("contractResults", TestType::Class("ResultBlock".to_owned())),
        TestStruct::new("ContractTemplate"),
        TestStruct::new("StarMapObject"),
        TestStruct::new("ResultBlock").array("contractResults", pointer("Result")),
        TestStruct::new("Result"),
        TestStruct::new("ContractResult_LegacyReputation")
            .parent("Result")
            .array("missionResults", TestType::Bool)
            .array(
                "contractResultReputationAmounts",
                TestType::Class("Amount".to_owned()),
            ),
        TestStruct::new("Amount").field("reward", TestType::Reference),
        TestStruct::new("Reward").field("reputationAmount", TestType::Int64),
        TestStruct::new("Broken")
            .field("value", TestType::Int32)
            .declared_size(6),
    ]
    .into_iter()
    .fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn contract() -> TestInstance {
    TestInstance::new("Contract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", TestValue::text("Selected"))
}

fn generator(contract: TestInstance) -> TestRecord {
    TestRecord::new(
        OWNER,
        "ContractGenerator",
        "ContractGenerator.Groups",
        "libs/foundry/records/contracts/contractgenerator/groups.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(TestInstance::new("Handler").value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        ))]),
    )
}

fn reward(guid: u128, name: &str, amount: i64) -> TestRecord {
    TestRecord::new(
        guid,
        "Reward",
        format!("Reward.{name}"),
        format!("{REWARDS}{name}.xml"),
    )
    .value("reputationAmount", TestValue::Int64(amount))
}
