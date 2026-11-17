// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x7100_0000_0000_4000_8000_0000_0000_0001;
const CONTRACT: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0001;
const LOCATION: u128 = 0x7300_0000_0000_4000_8000_0000_0000_0001;
pub(super) const ACCEPTED_KEY: &str = "@canonical_place";
pub(super) const REJECTED_KEY: &str = "@excluded_place";

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    OutsideOnly,
    OutsideShadow,
    LeftoverShadow,
}

pub(super) fn database(case: Case) -> Vec<u8> {
    let source = schema()
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(generator());
    let source = match case {
        Case::OutsideOnly => source.record(object("elsewhere/outside", "Outside", REJECTED_KEY)),
        Case::OutsideShadow => source
            .record(object("pu/accepted", "Accepted", ACCEPTED_KEY))
            .record(object("elsewhere/outside", "Outside", REJECTED_KEY)),
        Case::LeftoverShadow => source
            .record(object("pu/accepted", "Accepted", ACCEPTED_KEY))
            .record(object("pu/dev_place", "dev_place", REJECTED_KEY)),
    };
    source.build()
}

fn schema() -> [TestStruct; 6] {
    [
        TestStruct::new("ContractGenerator").array("generators", pointer("Handler")),
        TestStruct::new("Handler").array("contracts", pointer("Contract")),
        TestStruct::new("Contract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("additionalPrerequisites", pointer("ContractPrerequisite")),
        TestStruct::new("ContractPrerequisite"),
        TestStruct::new("ContractPrerequisite_Location")
            .parent("ContractPrerequisite")
            .field("locationAvailable", TestType::Reference),
        TestStruct::new("StarMapObject").field("name", TestType::Locale),
    ]
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn generator() -> TestRecord {
    let contract = TestInstance::new("Contract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", TestValue::text("Delivery"))
        .value(
            "additionalPrerequisites",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("ContractPrerequisite_Location")
                    .value("locationAvailable", TestValue::Reference(LOCATION)),
            )]),
        );
    TestRecord::new(
        OWNER,
        "ContractGenerator",
        "ContractGenerator.Carrier",
        "libs/foundry/records/contracts/contractgenerator/carrier.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(TestInstance::new("Handler").value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        ))]),
    )
}

fn object(path: &str, name: &str, key: &str) -> TestRecord {
    TestRecord::new(
        LOCATION,
        "StarMapObject",
        format!("StarMapObject.{name}"),
        format!("libs/foundry/records/starmap/{path}.xml"),
    )
    .value("name", TestValue::text(key))
}
