// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestStruct, TestType, TestValue};

use crate::flow_final_fixture as fixture;

pub(crate) fn structure(owner: TestStruct) -> TestStruct {
    owner
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
        .field("enum", TestType::Enum("FlowLiteralEnum".to_owned()))
        .field("class", TestType::Class("ExtraOwned".to_owned()))
        .field("strong", TestType::StrongPointer("ExtraOwned".to_owned()))
        .field("weak", TestType::WeakPointer("ExtraOwned".to_owned()))
        .field("reference", TestType::Reference)
}

pub(crate) fn values(name: &str, nan: bool) -> TestInstance {
    pointers(
        TestInstance::new(name)
            .value("flag", TestValue::Bool(false))
            .value("i8", TestValue::Int8(i8::MIN))
            .value("i16", TestValue::Int16(i16::MIN))
            .value("i32", TestValue::Int32(i32::MIN))
            .value("i64", TestValue::Int64(i64::MIN))
            .value("u8", TestValue::UInt8(u8::MAX))
            .value("u16", TestValue::UInt16(u16::MAX))
            .value("u32", TestValue::UInt32(u32::MAX))
            .value("u64", TestValue::UInt64(u64::MAX))
            .value("text", TestValue::text(fixture::TEXT))
            .value(
                "single",
                TestValue::Single(if nan {
                    f32::from_bits(fixture::NAN)
                } else {
                    -0.0
                }),
            )
            .value("double", TestValue::Double(-0.0))
            .value("locale", TestValue::text(fixture::LOCALE))
            .value("guid", TestValue::Guid(0))
            .value("enum", TestValue::text(fixture::ENUM)),
    )
}

fn pointers(value: TestInstance) -> TestInstance {
    value
        .value(
            "class",
            TestValue::Struct(
                TestInstance::new("ExtraOwned").value("marker", TestValue::Int32(-9)),
            ),
        )
        .value(
            "strong",
            TestValue::Struct(
                TestInstance::new("ExtraOwned").value("marker", TestValue::Int32(-10)),
            ),
        )
        .value("weak", TestValue::pointer("outside-original"))
        .value(
            "reference",
            TestValue::Reference(0xf160_0000_0000_4000_8000_0000_0000_0002),
        )
}
