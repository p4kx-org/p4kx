// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::test_support::TestResult;

pub(super) fn bytes() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Owner")
                .field("value", TestType::Bool)
                .field("bad", TestType::Bool)
                .array("items", TestType::Bool)
                .field("nested", TestType::StrongPointer("Mystery".to_owned())),
        )
        .structure(TestStruct::new("Mystery").field("payload", TestType::String));
    for index in 0..2_u128 {
        builder = builder.record(
            TestRecord::new(
                0xf1a0_0000_0000_4000_8000_0000_0000_0001 + index,
                "Owner",
                format!("Owner.Guard{index}"),
                format!("guard{index}.xml"),
            )
            .value("value", TestValue::Bool(true))
            .value("bad", TestValue::Raw(vec![2]))
            .value(
                "items",
                TestValue::Array(vec![TestValue::Bool(false), TestValue::Bool(true)]),
            )
            .value(
                "nested",
                TestValue::Struct(
                    TestInstance::new("Mystery").value("payload", TestValue::text("kept")),
                ),
            ),
        );
    }
    builder.build()
}

pub(super) fn original(tree: &RecordTree<'_>) -> TestResult {
    assert_eq!(
        tree.error_count(),
        1,
        "one actual malformed bool occurrence"
    );
    assert_eq!(
        tree.field("value"),
        Some(&Value::Bool(true)),
        "readable genuine boolean"
    );
    assert_eq!(
        tree.field("bad"),
        Some(&Value::Error(ValueError::InvalidBool { byte: 2 })),
        "exact original malformed bool byte"
    );
    let Some(Value::Array(values)) = tree.field("items") else {
        return Err("actual Bool array".into());
    };
    assert_eq!(
        values.as_ref(),
        [Value::Bool(false), Value::Bool(true)],
        "both real ordered array elements exist before the guard"
    );
    Ok(())
}
