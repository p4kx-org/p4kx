// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const OWNER: &str = "e9400000-0000-4000-8000-000000000001";
pub(crate) const TARGET: &str = "e9400000-0000-4000-8000-000000000002";
pub(crate) const OWNER_PATH: &str = "libs/foundry/records/lootgeneration/filters/owner";
pub(crate) const TARGET_PATH: &str = "libs/foundry/records/lootgeneration/filters/target";

pub(crate) fn open(input: Input) -> TestResult<Fixture> {
    Fixture::open(&database(), input, Vec::new())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owned").field("number", TestType::Int32))
        .structure(schema())
        .record(owner())
        .record(TestRecord::new(
            0xe940_0000_0000_4000_8000_0000_0000_0002,
            "PoolFilterRecord",
            "PoolFilterRecord.Target",
            format!("{TARGET_PATH}.xml"),
        ))
        .build()
}

fn schema() -> TestStruct {
    TestStruct::new("PoolFilterRecord")
        .field("owned", TestType::StrongPointer("Owned".to_owned()))
        .field("first", TestType::Reference)
        .field("second", TestType::Reference)
        .field(
            "weakRecord",
            TestType::WeakPointer("PoolFilterRecord".to_owned()),
        )
        .field(
            "weakRecordAgain",
            TestType::WeakPointer("PoolFilterRecord".to_owned()),
        )
        .field("weakInstance", TestType::WeakPointer("Owned".to_owned()))
        .field(
            "weakInstanceAgain",
            TestType::WeakPointer("Owned".to_owned()),
        )
        .field("nullStrong", TestType::StrongPointer("Owned".to_owned()))
        .field("nullWeak", TestType::WeakPointer("Owned".to_owned()))
        .field("nullReference", TestType::Reference)
}

fn owner() -> TestRecord {
    TestRecord::new(
        0xe940_0000_0000_4000_8000_0000_0000_0001,
        "PoolFilterRecord",
        "PoolFilterRecord.Owner",
        format!("{OWNER_PATH}.xml"),
    )
    .value(
        "owned",
        TestValue::Struct(
            TestInstance::new("Owned")
                .named("owned-original")
                .value("number", TestValue::Int32(11)),
        ),
    )
    .value(
        "first",
        TestValue::Reference(0xe940_0000_0000_4000_8000_0000_0000_0002),
    )
    .value(
        "second",
        TestValue::Reference(0xe940_0000_0000_4000_8000_0000_0000_0002),
    )
    .value("weakRecord", TestValue::pointer("PoolFilterRecord.Target"))
    .value(
        "weakRecordAgain",
        TestValue::pointer("PoolFilterRecord.Target"),
    )
    .value("weakInstance", TestValue::pointer("owned-original"))
    .value("weakInstanceAgain", TestValue::pointer("owned-original"))
}
