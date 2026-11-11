// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::test_support::TestResult;

pub(super) fn bytes() -> Vec<u8> {
    let params = || {
        TestValue::Struct(
            TestInstance::new("Params")
                .value("end", TestValue::Int32(9))
                .value("link", TestValue::Null),
        )
    };
    let owner = || {
        TestRecord::new(0xEC14, "Owner", "Owner.Absence", "original/absence.xml")
            .value("flag", TestValue::Bool(true))
            .value("nested", params())
            .value("other", params())
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Owner")
                .field("flag", TestType::Bool)
                .field("nested", TestType::StrongPointer("Params".to_owned()))
                .field("other", TestType::StrongPointer("Params".to_owned())),
        )
        .structure(
            TestStruct::new("Params")
                .field("end", TestType::Int32)
                .field("link", TestType::ReferenceTo("Owner".to_owned())),
        )
        .record(owner())
        .record(owner())
        .build()
}

pub(super) fn original<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    name: &str,
) -> TestResult<&'tree StructValue<'source>> {
    assert_eq!(
        tree.error_count(),
        0,
        "the original read has no source errors"
    );
    assert_eq!(tree.type_name(), "Owner", "the actual root concrete type");
    assert_eq!(
        tree.field("flag"),
        Some(&Value::Bool(true)),
        "the preceding original field"
    );
    let Some(Value::Struct(params)) = tree.field(name) else {
        return Err("actual nested Params".into());
    };
    assert_eq!(
        params.type_name(),
        "Params",
        "the actual nested type differs from its root"
    );
    let names: Vec<_> = params
        .fields()
        .iter()
        .map(p4kx_extraction::Field::name)
        .collect();
    assert_eq!(
        names,
        ["end", "link"],
        "the complete original declaration list independently proves absence"
    );
    assert_eq!(
        params.field("end"),
        Some(&Value::Int32(9)),
        "the neighboring authored numeric value"
    );
    assert_eq!(
        params.field("link"),
        Some(&Value::Null),
        "a declared Null is present in the original list"
    );
    assert!(
        params.field("start").is_none(),
        "the requested fixed required name is not declared"
    );
    Ok(params)
}
