// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const OWNER: &str = "e9700000-0000-4000-8000-000000000001";
pub(crate) const TARGET: &str = "e9700000-0000-4000-8000-000000000002";
pub(crate) const TEXT: &str = "  @literal.description %.2f M\tline\n'quote'\\path  ";
pub(crate) const LOCALE: &str = "@unclassified.original.key";
pub(crate) const ENUM: &str = "@enum.is.a.literal";
pub(crate) const TEMPLATE: &str = "%+.2f %%";

pub(crate) fn open(input: Input) -> TestResult<Fixture> {
    Fixture::open(&database(), input, Vec::new())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("LiteralEnum", [ENUM])
        .structure(
            TestStruct::new("CraftingGameplayPropertyDef")
                .field("propertyName", TestType::String)
                .field("unitFormat", TestType::Locale),
        )
        .structure(
            TestStruct::new("PoolFilterInstance")
                .field("name", TestType::String)
                .field("mode", TestType::String),
        )
        .structure(schema())
        .record(record())
        .record(TestRecord::new(
            0xe970_0000_0000_4000_8000_0000_0000_0002,
            "PoolFilterRecord",
            "PoolFilterRecord.Target",
            "target.xml",
        ))
        .build()
}

fn schema() -> TestStruct {
    TestStruct::new("PoolFilterRecord")
        .field("flag", TestType::Bool)
        .field("i8", TestType::Int8)
        .field("i16", TestType::Int16)
        .field("i32", TestType::Int32)
        .field("i64", TestType::Int64)
        .field("u8", TestType::UInt8)
        .field("u16", TestType::UInt16)
        .field("u32", TestType::UInt32)
        .field("u64", TestType::UInt64)
        .field("text", TestType::String)
        .field("single", TestType::Single)
        .field("double", TestType::Double)
        .field("locale", TestType::Locale)
        .field("guid", TestType::Guid)
        .field("enum", TestType::Enum("LiteralEnum".to_owned()))
        .field(
            "class",
            TestType::Class("CraftingGameplayPropertyDef".to_owned()),
        )
        .field(
            "strong",
            TestType::StrongPointer("PoolFilterInstance".to_owned()),
        )
        .field(
            "weak",
            TestType::WeakPointer("PoolFilterInstance".to_owned()),
        )
        .field("reference", TestType::Reference)
        .field("unitFormat", TestType::String)
        .field("empty", TestType::String)
        .field("signedMax", TestType::Int64)
}

fn record() -> TestRecord {
    integers(TestRecord::new(
        0xe970_0000_0000_4000_8000_0000_0000_0001,
        "PoolFilterRecord",
        "PoolFilterRecord.AllTypes",
        "libs/foundry/records/lootgeneration/filters/alltypes.xml",
    ))
    .value("text", TestValue::text(TEXT))
    .value("single", TestValue::Single(-0.0))
    .value("double", TestValue::Double(-0.0))
    .value("locale", TestValue::text(LOCALE))
    .value("guid", TestValue::Guid(0))
    .value("enum", TestValue::text(ENUM))
    .value(
        "class",
        TestValue::Struct(
            TestInstance::new("CraftingGameplayPropertyDef")
                .value("propertyName", TestValue::text("temperature"))
                .value("unitFormat", TestValue::text("%.2f RPM")),
        ),
    )
    .value(
        "strong",
        TestValue::Struct(
            TestInstance::new("PoolFilterInstance")
                .named("actual-strong")
                .value("name", TestValue::text("@designer.literal"))
                .value("mode", TestValue::text("Cumulative")),
        ),
    )
    .value("weak", TestValue::pointer("actual-strong"))
    .value(
        "reference",
        TestValue::Reference(0xe970_0000_0000_4000_8000_0000_0000_0002),
    )
    .value("unitFormat", TestValue::text(TEMPLATE))
    .value("empty", TestValue::text(""))
    .value("signedMax", TestValue::Int64(i64::MAX))
}

fn integers(record: TestRecord) -> TestRecord {
    record
        .value("flag", TestValue::Bool(false))
        .value("i8", TestValue::Int8(i8::MIN))
        .value("i16", TestValue::Int16(i16::MIN))
        .value("i32", TestValue::Int32(i32::MIN))
        .value("i64", TestValue::Int64(i64::MIN))
        .value("u8", TestValue::UInt8(u8::MAX))
        .value("u16", TestValue::UInt16(u16::MAX))
        .value("u32", TestValue::UInt32(u32::MAX))
        .value("u64", TestValue::UInt64(u64::MAX))
}
