// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataType, SourceLocation, SourceUseRole, SourceUses, SourcedValue, Value, ValueError,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

use super::super::error::OriginalValueErrorFacts;
use super::super::error_facts::ErrorFacts;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueEvent, OriginalValueFact};

#[derive(Clone, Copy)]
enum Owner {
    Root,
    Strong,
    Array,
}

#[test]
fn inline_cycles_use_the_actual_non_array_class_target_selector() -> TestResult {
    for owner in [Owner::Root, Owner::Strong, Owner::Array] {
        test_support::with_input(&bytes(owner), |forge, input| {
            declarations(forge)?;
            let record = forge.record(0).ok_or("original cycle owner")?;
            let tree = record.tree()?;
            assert!(tree.is_from(&record), "same original read owner");
            assert_eq!(
                tree.error_count(),
                2,
                "both inline declarations fail independently"
            );
            let scope = input.record_scope(&record, &tree)?;
            occurrences(&scope, owner)?;
            Ok(())
        })?;
    }
    Ok(())
}

fn occurrences(scope: &OriginalRecordScope<'_, '_>, owner: Owner) -> TestResult {
    let mut walk = scope.walk(None);
    let mut errors = 0;
    while let Some(event) = walk.next_event()? {
        let OriginalValueEvent::Value(site) = event else {
            continue;
        };
        let fact = site.fact();
        if matches!(
            site.value(),
            Value::Error(ValueError::InlineClassCycle { .. })
        ) {
            let prefix = match owner {
                Owner::Root => 0,
                Owner::Strong => 1,
                Owner::Array => 2,
            };
            original_error(&fact, scope.sources(), prefix + errors)?;
            errors += 1;
        } else {
            assert!(
                fact.check_inline_class_cycle(0).is_err(),
                "a container, pointer or class-array declaration cannot claim this creator"
            );
        }
    }
    assert_eq!(errors, 2, "every original failed occurrence was captured");
    Ok(())
}

fn declarations(forge: &DataForge<'_>) -> TestResult {
    let definition = forge.structs().next().ok_or("original Loop declaration")?;
    assert_eq!(definition.name()?, "Loop", "actual original mapping");
    assert_eq!(
        definition.size(),
        0,
        "actual explicit zero-width source declaration"
    );
    assert_eq!(definition.instance_count(), 1, "one physical Loop instance");
    let properties: Vec<_> = definition.properties().collect();
    assert_eq!(properties.len(), 2, "both actual source declarations");
    for (field, expected) in properties.iter().zip(["left", "right"]) {
        assert_eq!(field.name()?, expected, "original declaration order");
        assert_eq!(
            field.data_type()?,
            DataType::Class,
            "original inline storage kind"
        );
        assert!(!field.is_array(), "actual cyclic member is not an array");
        assert_eq!(
            field
                .target_struct()
                .ok_or("actual original target")?
                .index(),
            0,
            "both original declaration edges choose Loop"
        );
    }
    Ok(())
}
fn original_error(
    fact: &OriginalValueFact<'_, '_, '_>,
    sources: &SourceUses,
    ordinal: u64,
) -> TestResult {
    let storage = fact.field().storage();
    assert_eq!(
        storage.data_type(),
        DataType::Class,
        "actual declaration kind"
    );
    assert!(
        !storage.is_array(),
        "the cyclic declaration is scalar Class"
    );
    assert_eq!(storage.target(), 0, "the actual self type");
    assert!(
        ["left", "right"].contains(&fact.field().name()),
        "exact original fields"
    );
    let OriginalValueErrorFacts::InlineClassCycle { struct_index } = super::read(fact, sources)?
    else {
        return Err("typed original cycle facts".into());
    };
    assert_eq!(
        struct_index.value(),
        Some(&0),
        "exact original declaration index"
    );
    assert_eq!(
        fact.ordinal(),
        ordinal,
        "the actual root-fields-first source position"
    );
    exact_origin(&struct_index, sources, ordinal)?;
    let before = sources.freeze()?.uses().len();
    let facts = ErrorFacts {
        site: Some(fact),
        sources,
    };
    assert!(
        facts.inline_class(1).is_err(),
        "a different target index is not this error input"
    );
    assert_eq!(
        sources.freeze()?.uses().len(),
        before,
        "rejected declaration mints no use"
    );
    Ok(())
}

fn exact_origin(cell: &SourcedValue<u32>, sources: &SourceUses, ordinal: u64) -> TestResult {
    let origin = cell.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "plain actual input"
    );
    assert!(
        origin.source_set_complete(),
        "the exact declaration input is retained"
    );
    let [reference] = origin.source_uses_refs() else {
        return Err("one precise inline-cycle declaration contribution".into());
    };
    let inventory = sources.freeze()?;
    let observed = inventory
        .uses()
        .iter()
        .find(|row| row.id() == reference)
        .ok_or("original selector exists in this inventory")?;
    assert_eq!(
        observed.role(),
        SourceUseRole::Selector,
        "declaration chooses inline type"
    );
    assert_eq!(
        observed.location(),
        &SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![0, ordinal],
            field: "$target".to_owned(),
        },
        "exact original declaration, no fabricated pointer word or error-parameter payload"
    );
    assert_eq!(
        serde_json::to_value(cell)?.get("value"),
        Some(&json!(0)),
        "wire preserves the real index independently of origin"
    );
    Ok(())
}

fn bytes(owner: Owner) -> Vec<u8> {
    let database = DataForgeBuilder::new().version(8).structure(
        TestStruct::new("Loop")
            .field("left", TestType::Class("Loop".to_owned()))
            .field("right", TestType::Class("Loop".to_owned()))
            .declared_size(0),
    );
    let record = |kind| TestRecord::new(0xcee1, kind, "Cycle.One", "cycle.xml");
    match owner {
        Owner::Root => database.record(record("Loop")),
        Owner::Strong => database
            .structure(
                TestStruct::new("Owner").field("child", TestType::StrongPointer("Loop".to_owned())),
            )
            .instance("loop", TestInstance::new("Loop"))
            .record(record("Owner").value("child", TestValue::pointer("loop"))),
        Owner::Array => database
            .structure(TestStruct::new("Owner").array("child", TestType::Class("Loop".to_owned())))
            .record(record("Owner").value(
                "child",
                TestValue::Array(vec![TestValue::Struct(TestInstance::new("Loop"))]),
            )),
    }
    .build()
}
