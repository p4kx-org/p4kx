// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const CHAIN: u128 = 0xe900_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TARGET: u128 = 0xe900_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const EMPTY: u128 = 0xe900_0000_0000_4000_8000_0000_0000_0003;

pub(crate) fn database(layers: usize) -> Vec<u8> {
    let mut filter = TestInstance::new("PoolFilter_RecordRef")
        .value("filterRecord", TestValue::Reference(TARGET));
    for ordinal in (0..layers).rev() {
        let child = TestInstance::new("PoolFilterInstance")
            .value("name", TestValue::text(format!("step-{ordinal}")))
            .value("mode", TestValue::text("Cumulative"))
            .value("filter", TestValue::Struct(filter));
        filter = TestInstance::new("PoolFilter_Sequence")
            .value("filters", TestValue::Array(vec![TestValue::Struct(child)]));
    }
    schema()
        .record(record(CHAIN, "chain").value("filter", TestValue::Struct(filter)))
        .record(record(TARGET, "target"))
        .record(
            record(EMPTY, "empty").value(
                "filter",
                TestValue::Struct(
                    TestInstance::new("PoolFilter_Sequence")
                        .value("filters", TestValue::Array(Vec::new())),
                ),
            ),
        )
        .build()
}

fn schema() -> DataForgeBuilder {
    let mut database = DataForgeBuilder::new().version(8);
    for structure in [
        TestStruct::new("PoolFilter"),
        TestStruct::new("PoolFilterInstance")
            .field("name", TestType::String)
            .field("mode", TestType::String)
            .field("filter", TestType::StrongPointer("PoolFilter".into())),
        TestStruct::new("PoolFilter_Sequence")
            .parent("PoolFilter")
            .array("filters", TestType::Class("PoolFilterInstance".into())),
        TestStruct::new("PoolFilter_RecordRef")
            .parent("PoolFilter")
            .field("filterRecord", TestType::Reference),
        TestStruct::new("PoolFilterRecord")
            .field("filter", TestType::StrongPointer("PoolFilter".into())),
    ] {
        database = database.structure(structure);
    }
    database
}

fn record(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "PoolFilterRecord",
        format!("PoolFilterRecord.{name}"),
        format!("libs/foundry/records/lootgeneration/filters/{name}.xml"),
    )
}
