// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(super) fn database(malformed: bool) -> Vec<u8> {
    let owner = TestStruct::new("ContractTemplate").field("value", TestType::UInt32);
    let owner = if malformed {
        owner.declared_size(5)
    } else {
        owner
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(owner)
        .structure(TestStruct::new("Other"))
        .record(record(0x2222, "Second", "kept/two.xml", 22))
        .record(record(0x1111, "First", "kept/one.xml", 11))
        .record(record(0x1111, "Alias", "kept/alias.xml", 33))
        .record(record(0x3333, "Dev", "kept/dev.xml", 44))
        .record(TestRecord::new(
            0x4444,
            "Other",
            "Other.Outside",
            "libs/foundry/records/kept/wrong.xml",
        ))
        .build()
}

fn record(guid: u128, name: &str, path: &str, value: u32) -> TestRecord {
    TestRecord::new(
        guid,
        "ContractTemplate",
        format!("ContractTemplate.{name}"),
        format!("libs/foundry/records/{path}"),
    )
    .value("value", TestValue::UInt32(value))
}
