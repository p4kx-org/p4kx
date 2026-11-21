// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{DataForge, DataType, JsonLayout, RecordJson, RecordTree, RecordXml, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;
pub(crate) const OWNER: u128 = 0x5811;
pub(crate) const TARGET: u128 = 0x5812;
pub(crate) const MISSING: u128 = 0x5813;
pub(crate) const INVALID_STRUCT: u32 = 77;
pub(crate) const INVALID_INSTANCE: u32 = 99;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Reference,
    Weak,
}

impl Case {
    pub(crate) const fn data_type(self) -> DataType {
        match self {
            Self::Reference => DataType::Reference,
            Self::Weak => DataType::WeakPointer,
        }
    }

    fn test_type(self) -> TestType {
        match self {
            Self::Reference => TestType::ReferenceTo("Target".to_owned()),
            Self::Weak => TestType::WeakPointer("Target".to_owned()),
        }
    }

    fn target(self) -> TestValue {
        match self {
            Self::Reference => TestValue::Reference(TARGET),
            Self::Weak => TestValue::pointer("Target.One"),
        }
    }
}

pub(crate) fn database(case: Case, array: bool) -> Vec<u8> {
    let value = if array {
        TestValue::Array(vec![case.target(), TestValue::Null, case.target()])
    } else {
        case.target()
    };
    builder(case, array, value).build()
}

pub(crate) fn builder(case: Case, array: bool, value: TestValue) -> DataForgeBuilder {
    let structure = TestStruct::new("Owner");
    let structure = if array {
        structure.array("Link", case.test_type())
    } else {
        structure.field("Link", case.test_type())
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Target"))
        .structure(structure)
        .record(TestRecord::new(OWNER, "Owner", "Owner.One", "owner.xml").value("Link", value))
        .record(TestRecord::new(
            TARGET,
            "Target",
            "Target.One",
            "target.xml",
        ))
}

pub(crate) fn original(forge: &DataForge<'_>, case: Case, array: bool) -> TestResult {
    let owner = forge.record(0).ok_or("original owning Record")?;
    assert_eq!(
        owner.guid().to_string(),
        "00000000-0000-0000-0000-000000005811",
        "actual original owner GUID"
    );
    let structure = owner.struct_type()?;
    let declarations: Vec<_> = structure.properties().collect();
    assert_eq!(declarations.len(), 1, "one actual source declaration");
    let declaration = declarations.first().ok_or("original Link declaration")?;
    assert_eq!(
        declaration.name()?,
        "Link",
        "the original field name is exact"
    );
    assert_eq!(
        declaration.data_type()?,
        case.data_type(),
        "actual source storage kind"
    );
    assert_eq!(
        declaration.is_array(),
        array,
        "actual source conversion kind"
    );
    let target = declaration
        .target_struct()
        .ok_or("actual declared target")?;
    assert_eq!(
        target.index(),
        0,
        "the original target word names the first struct"
    );
    assert_eq!(
        target.name()?,
        "Target",
        "the actual declared target is preserved"
    );
    let record = forge.record(1).ok_or("original target Record")?;
    assert_eq!(
        record.guid().to_string(),
        "00000000-0000-0000-0000-000000005812",
        "actual original target GUID"
    );
    assert_eq!(
        record.struct_type()?.index(),
        target.index(),
        "the original target has the declared type"
    );
    assert_eq!(
        record.record_path()?,
        "target",
        "the original target path is independently read"
    );
    Ok(())
}

pub(crate) fn equal_links(
    left: &RecordTree<'_>,
    right: &RecordTree<'_>,
    array: bool,
) -> TestResult {
    let left = left.field("Link").ok_or("actual Reference field")?;
    let right = right.field("Link").ok_or("actual WeakPointer field")?;
    assert_eq!(
        left, right,
        "decoded values alone erase the actual storage distinction"
    );
    let values: &[Value<'_>] = if array {
        let Value::Array(values) = left else {
            return Err("original link array".into());
        };
        assert_eq!(
            values.len(),
            3,
            "original target/null/repeated-target order"
        );
        assert_eq!(
            values.get(1),
            Some(&Value::Null),
            "the original null slot survives"
        );
        assert_eq!(
            values.first(),
            values.get(2),
            "a repeated original target is retained"
        );
        values.as_ref()
    } else {
        std::slice::from_ref(left)
    };
    for value in values {
        if matches!(value, Value::Null) {
            continue;
        }
        let Value::Reference(reference) = value else {
            return Err("actual resolved link".into());
        };
        assert_eq!(
            reference.record_index(),
            1,
            "the same physical target was resolved"
        );
        assert_eq!(
            reference.record_path(),
            "target",
            "the same target header was resolved"
        );
        assert_eq!(
            reference.type_name(),
            "Target",
            "the actual target type is equal"
        );
        assert_eq!(
            reference.location(),
            None,
            "a weak pointer to a Record has no nested location"
        );
    }
    Ok(())
}

pub(crate) fn rendering(tree: &RecordTree<'_>) -> TestResult<(Vec<u8>, Vec<u8>)> {
    let mut json = Vec::new();
    RecordJson::new(JsonLayout::Compact).write(tree, &mut json)?;
    let mut xml = Vec::new();
    RecordXml::new().write(tree, &mut xml)?;
    Ok((json, xml))
}
