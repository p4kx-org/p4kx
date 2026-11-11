// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const GUID: &str = "e9500000-0000-4000-8000-000000000001";

pub(crate) fn open(nodes: usize) -> TestResult<Fixture> {
    let mut record = TestRecord::new(
        0xe950_0000_0000_4000_8000_0000_0000_0001,
        "PoolFilterRecord",
        "PoolFilterRecord.Depth",
        "libs/foundry/records/lootgeneration/filters/depth.xml",
    );
    if nodes > 0 {
        record = record.value("filter", TestValue::pointer("node0"));
    }
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("DepthNode")
                .field("marker", TestType::UInt32)
                .field("next", TestType::StrongPointer("DepthNode".to_owned())),
        )
        .structure(
            TestStruct::new("PoolFilterRecord")
                .field("filter", TestType::StrongPointer("DepthNode".to_owned())),
        )
        .record(record);
    for ordinal in 0..nodes {
        let mut instance = TestInstance::new("DepthNode")
            .value("marker", TestValue::UInt32(u32::try_from(ordinal)?));
        let next = ordinal
            .checked_add(1)
            .ok_or("synthetic source ordinal overflow")?;
        if next < nodes {
            instance = instance.value("next", TestValue::pointer(format!("node{next}")));
        }
        database = database.instance(format!("node{ordinal}"), instance);
    }
    Fixture::open(&database.build(), Input::Plain, Vec::new())
}
