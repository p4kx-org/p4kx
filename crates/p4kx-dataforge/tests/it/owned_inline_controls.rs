// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};

use crate::owned_source_fixture::TestResult;

#[test]
fn repeated_acyclic_empty_inline_classes_remain_distinct_valid_occurrences() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    original(&forge)?;
    let tree = forge.record(0).ok_or("original inline owner")?.tree()?;
    for field in ["left", "right"] {
        let Some(Value::Struct(pair)) = tree.field(field) else {
            return Err("both original Pair occurrences must survive".into());
        };
        assert_eq!(pair.type_name(), "Pair", "the original intermediate class");
        for field in ["first", "second"] {
            let Some(Value::Struct(empty)) = pair.field(field) else {
                return Err("all four original Empty occurrences must survive".into());
            };
            assert_eq!(
                empty.type_name(),
                "Empty",
                "the original repeated empty class"
            );
            assert!(
                empty.fields().is_empty(),
                "the original empty declaration has no invented members"
            );
        }
    }
    assert_eq!(
        tree.error_count(),
        0,
        "zero byte size and sibling reuse are not inline cycles"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty"))
        .structure(
            TestStruct::new("Pair")
                .field("first", TestType::Class("Empty".to_owned()))
                .field("second", TestType::Class("Empty".to_owned())),
        )
        .structure(
            TestStruct::new("InlineOwner")
                .field("left", TestType::Class("Pair".to_owned()))
                .field("right", TestType::Class("Pair".to_owned())),
        )
        .record(TestRecord::new(
            0x7113,
            "InlineOwner",
            "InlineOwner.Source",
            "source/inline.xml",
        ))
        .build()
}

fn original(forge: &DataForge<'_>) -> TestResult {
    assert_eq!(
        forge.structs().len(),
        3,
        "three genuine original inline declarations"
    );
    for ((index, structure), expected_instances) in forge.structs().enumerate().zip([0, 0, 1]) {
        assert_eq!(
            structure.size(),
            0,
            "these original inline instances occupy no bytes"
        );
        assert_eq!(
            structure.instance_count(),
            expected_instances,
            "only the original owner has a mapped physical instance"
        );
        for property in structure.properties() {
            assert_eq!(
                property.data_type()?,
                DataType::Class,
                "a genuine inline class declaration"
            );
            assert!(!property.is_array(), "no owned array is involved");
            assert!(
                usize::try_from(
                    property
                        .target_struct()
                        .ok_or("original inline target")?
                        .index()
                )? < index,
                "every actual inline edge points strictly earlier, so the schema is acyclic"
            );
        }
    }
    let survey = forge.survey()?;
    assert_eq!(
        survey.unreadable_structs, 0,
        "independent original layouts are readable"
    );
    Ok(())
}
