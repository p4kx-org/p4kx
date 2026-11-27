// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const SINGLE: u32 = 0x7fc0_1234;
pub(super) const DOUBLE: u64 = 0x7ff8_0000_0000_abcd;

pub(super) fn bytes() -> Vec<u8> {
    bytes_owner(
        "ContractTemplate",
        "ContractTemplate.Numbers",
        "libs/foundry/records/contracts/contracttemplates/numbers.xml",
    )
}

pub(super) fn broker_bytes() -> Vec<u8> {
    bytes_owner(
        "MissionBrokerEntry",
        "MissionBrokerEntry.Numbers",
        "libs/foundry/records/missionbroker/observed/numbers.xml",
    )
}

fn bytes_owner(owner: &str, name: &str, path: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(owner)
                .field("missionFlow", TestType::Class("MissionFlow".to_owned()))
                .field("outside", TestType::Single),
        )
        .structure(
            TestStruct::new("MissionFlow")
                .array(
                    "triggers",
                    TestType::StrongPointer("FlowTrigger".to_owned()),
                )
                .array("equal", TestType::Single)
                .field("single", TestType::Single)
                .field("double", TestType::Double),
        )
        .structure(TestStruct::new("FlowTrigger"))
        .record(
            TestRecord::new(0xf1c0_0000_0000_4000_8000_0000_0000_0001, owner, name, path)
                .value(
                    "missionFlow",
                    TestValue::Struct(
                        TestInstance::new("MissionFlow")
                            .value("triggers", TestValue::Array(Vec::new()))
                            .value(
                                "equal",
                                TestValue::Array(vec![
                                    TestValue::Single(f32::from_bits(SINGLE)),
                                    TestValue::Single(f32::from_bits(SINGLE)),
                                ]),
                            )
                            .value("single", TestValue::Single(f32::from_bits(SINGLE)))
                            .value("double", TestValue::Double(f64::from_bits(DOUBLE))),
                    ),
                )
                .value("outside", TestValue::Single(f32::from_bits(SINGLE))),
        )
        .build()
}
