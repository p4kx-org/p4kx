// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(super) fn database(broken_globals: bool) -> DataForgeBuilder {
    let global = TestStruct::new("MiningGlobalParams").field("defaultMass", TestType::Single);
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(if broken_globals {
            global.declared_size(1)
        } else {
            global
        })
        .enumeration("ObservedSpeed", ["Fast"])
        .structure(
            TestStruct::new("RefiningProcess")
                .field("refiningSpeed", TestType::Enum("ObservedSpeed".to_owned())),
        )
        .structure(
            TestStruct::new("ResourceType")
                .declared_size(1)
                .field("value", TestType::UInt64),
        )
        .structure(
            TestStruct::new("Foreign")
                .declared_size(1)
                .field("value", TestType::UInt64),
        );
    builder
        .record(global_record(1, "primary"))
        .record(global_record(2, "sibling"))
        .record(process(3, "primary"))
        .record(process(4, "sibling"))
        .record(TestRecord::new(
            5,
            "ResourceType",
            "ResourceType.Canary",
            "libs/foundry/records/resourcetypedatabase/canary.xml",
        ))
        .record(TestRecord::new(
            6,
            "Foreign",
            "Foreign.Canary",
            "libs/foundry/records/unrelated/canary.xml",
        ))
        .record(global_record(1, "alias"))
        .record(process(7, "dev"))
}

fn global_record(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "MiningGlobalParams",
        format!("MiningGlobalParams.{name}"),
        format!("libs/foundry/records/mining/miningglobalparams_{name}.xml"),
    )
    .value("defaultMass", TestValue::Single(17.0))
}

fn process(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "RefiningProcess",
        format!("RefiningProcess.{name}"),
        format!("libs/foundry/records/refiningprocess/{name}.xml"),
    )
    .value("refiningSpeed", TestValue::text("Fast"))
}
