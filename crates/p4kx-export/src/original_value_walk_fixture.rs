// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(in super::super) fn database() -> Vec<u8> {
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Leaf").field("marker", TestType::UInt32))
        .structure(
            TestStruct::new("Nested")
                .field("zero", TestType::UInt32)
                .field("odd.name", TestType::String),
        )
        .structure(
            TestStruct::new("Owner")
                .field("dup", TestType::Bool)
                .field("dup", TestType::Bool)
                .field("container", TestType::Class("Nested".to_owned()))
                .array("slots", TestType::StrongPointer("Leaf".to_owned()))
                .field("tail", TestType::Bool),
        );
    for guid in [1, 2] {
        database = database.record(
            TestRecord::new(
                guid,
                "Owner",
                format!("Owner.{guid}"),
                format!("owner/{guid}.xml"),
            )
            .value(
                "container",
                TestValue::Struct(
                    TestInstance::new("Nested")
                        .value("zero", TestValue::UInt32(0))
                        .value("odd.name", TestValue::text("literal")),
                ),
            )
            .value(
                "slots",
                TestValue::Array(vec![
                    TestValue::Struct(
                        TestInstance::new("Leaf").value("marker", TestValue::UInt32(7)),
                    ),
                    TestValue::Null,
                    TestValue::Raw([77_u32.to_le_bytes(), 99_u32.to_le_bytes()].concat()),
                    TestValue::Struct(
                        TestInstance::new("Leaf").value("marker", TestValue::UInt32(7)),
                    ),
                ]),
            ),
        );
    }
    database.build()
}
