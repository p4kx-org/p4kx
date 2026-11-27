// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ptr;

use p4kx_extraction::{DataForge, DataType, RecordTree, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{owner_of, owner_of_any, struct_named};

type Checked<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn owner_lookup_keeps_a_materialized_field_beyond_sixteen_levels() -> Checked {
    for depth in [16, 17] {
        let bytes = chain(depth);
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("actual source Record")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the actual source tree keeps its owner"
        );
        let expected = original_leaf(&tree, depth)?;
        let found =
            owner_of(tree.root(), "DamagePhysical").ok_or("original damage owner was omitted")?;
        assert!(
            ptr::eq(found, expected),
            "the exact original owner is found at depth {depth}"
        );
    }
    Ok(())
}

#[test]
fn any_owner_lookup_keeps_a_materialized_field_beyond_sixteen_levels() -> Checked {
    for depth in [16, 17] {
        let bytes = chain(depth);
        let forge = DataForge::open(&bytes)?;
        let tree = forge.record(0).ok_or("actual source Record")?.tree()?;
        let expected = original_leaf(&tree, depth)?;
        let found = owner_of_any(tree.root(), &["DamageEnergy", "DamagePhysical"])
            .ok_or("original alternative damage owner was omitted")?;
        assert!(
            ptr::eq(found, expected),
            "source depth {depth} cannot change field presence"
        );
    }
    Ok(())
}

#[test]
fn named_lookup_keeps_a_materialized_type_beyond_thirty_two_levels() -> Checked {
    for depth in [32, 33] {
        let bytes = chain(depth);
        let forge = DataForge::open(&bytes)?;
        let tree = forge.record(0).ok_or("actual source Record")?.tree()?;
        let expected = original_leaf(&tree, depth)?;
        let found = struct_named(tree.root(), &format!("Layer{depth}"))
            .ok_or("original named structure was omitted")?;
        assert!(
            ptr::eq(found, expected),
            "the exact original type at depth {depth} is retained"
        );
    }
    Ok(())
}

#[test]
fn damage_projection_preserves_deep_readable_values() -> Checked {
    let bytes = chain(17);
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("actual source Record")?.tree()?;
    original_leaf(&tree, 17)?;
    let damage = crate::item_stats::parts::damage(Some(tree.root()))
        .ok_or("a readable original damage block must not become absent")?;
    assert_eq!(
        damage.physical,
        Some(77.0),
        "the real projection retains the stored damage value"
    );
    assert_eq!(
        damage.energy, None,
        "a genuinely absent neighbour remains absent"
    );
    Ok(())
}

fn chain(depth: usize) -> Vec<u8> {
    assert!(depth > 0, "the fixture has an owned child chain");
    let mut schema = DataForgeBuilder::new().version(8);
    for level in 0..depth {
        schema = schema.structure(
            TestStruct::new(format!("Layer{level}"))
                .field("next", TestType::Class(format!("Layer{}", level + 1))),
        );
    }
    let leaf_type = format!("Layer{depth}");
    schema =
        schema.structure(TestStruct::new(&leaf_type).field("DamagePhysical", TestType::Single));
    let mut child = TestInstance::new(&leaf_type).value("DamagePhysical", TestValue::Single(77.0));
    for level in (1..depth).rev() {
        child = TestInstance::new(format!("Layer{level}")).value("next", TestValue::Struct(child));
    }
    schema
        .record(
            TestRecord::new(0x7531, "Layer0", "Layer0.One", "lookup.xml")
                .value("next", TestValue::Struct(child)),
        )
        .build()
}

fn original_leaf<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    depth: usize,
) -> Checked<&'tree StructValue<'source>> {
    assert_eq!(
        tree.error_count(),
        0,
        "the canonical reader reached all original data without a limit/error"
    );
    let mut structure = tree.root();
    for level in 1..=depth {
        let field = structure
            .fields()
            .first()
            .ok_or("original next declaration")?;
        assert_eq!(
            field.name(),
            "next",
            "the independently constructed original path"
        );
        assert_eq!(
            field.storage().data_type(),
            DataType::Class,
            "actual inline storage"
        );
        assert!(
            !field.storage().is_array(),
            "this original path has one owned struct per step"
        );
        let Value::Struct(child) = field.value() else {
            return Err("original owned child".into());
        };
        assert_eq!(
            child.type_name(),
            format!("Layer{level}"),
            "actual source type/order"
        );
        structure = child;
    }
    assert_eq!(
        structure.field("DamagePhysical"),
        Some(&Value::Single(77.0)),
        "the original deep field is readable"
    );
    Ok(structure)
}

#[test]
fn stored_order_root_precedence_and_unfollowed_links_are_preserved() -> Checked {
    for root_value in [false, true] {
        let bytes = branches(root_value);
        let forge = DataForge::open(&bytes)?;
        let tree = forge.record(0).ok_or("actual branching owner")?.tree()?;
        let first = original_first_child(&tree)?;
        let expected = if root_value { tree.root() } else { first };
        for found in [
            owner_of(tree.root(), "DamagePhysical"),
            owner_of_any(tree.root(), &["DamagePhysical"]),
        ] {
            assert!(
                found.is_some_and(|found| ptr::eq(found, expected)),
                "root precedes children, then original array order wins"
            );
        }
        assert!(
            struct_named(tree.root(), "Leaf").is_some_and(|found| ptr::eq(found, first)),
            "named lookup keeps first array element"
        );
        assert_unfollowed_links(&tree, &forge)?;
    }
    Ok(())
}

fn original_first_child<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Checked<&'tree StructValue<'source>> {
    assert_eq!(
        tree.error_count(),
        0,
        "all original branching data is readable"
    );
    let Some(Value::Array(children)) = tree.field("children") else {
        return Err("original class array".into());
    };
    assert_eq!(children.len(), 2, "both original ordered children exist");
    let first = structure(children.first().ok_or("first original child")?)?;
    let second = structure(children.get(1).ok_or("second original child")?)?;
    assert_eq!(
        first.field("DamagePhysical"),
        Some(&Value::Single(10.0)),
        "first original child value"
    );
    assert_eq!(
        second.field("DamagePhysical"),
        Some(&Value::Single(20.0)),
        "second original child value"
    );
    Ok(first)
}

fn assert_unfollowed_links(tree: &RecordTree<'_>, forge: &DataForge<'_>) -> Checked {
    let target = forge
        .record(1)
        .ok_or("actual linked external Record")?
        .tree()?;
    assert_eq!(
        target.field("ExternalOnly"),
        Some(&Value::UInt32(99)),
        "the linked original target genuinely has the field"
    );
    assert!(
        matches!(tree.field("target"), Some(Value::Reference(_))),
        "the source holds a link rather than owned content"
    );
    assert!(
        owner_of(tree.root(), "ExternalOnly").is_none(),
        "links are not followed as owned descendants"
    );
    assert!(
        owner_of_any(tree.root(), &["Missing", "ExternalOnly"]).is_none(),
        "real absence remains absence"
    );
    assert!(
        struct_named(tree.root(), "External").is_none(),
        "named lookup does not follow links"
    );
    Ok(())
}

fn structure<'tree, 'source>(value: &'tree Value<'source>) -> Checked<&'tree StructValue<'source>> {
    match value {
        Value::Struct(value) => Ok(value),
        _ => Err("actual original owned structure".into()),
    }
}

fn branches(root_value: bool) -> Vec<u8> {
    let mut owner = TestStruct::new("Owner")
        .array("children", TestType::Class("Leaf".to_owned()))
        .field("target", TestType::Reference);
    let mut record = TestRecord::new(0x7532, "Owner", "Owner.One", "branches.xml")
        .value(
            "children",
            TestValue::Array(vec![
                TestValue::Struct(
                    TestInstance::new("Leaf").value("DamagePhysical", TestValue::Single(10.0)),
                ),
                TestValue::Struct(
                    TestInstance::new("Leaf").value("DamagePhysical", TestValue::Single(20.0)),
                ),
            ]),
        )
        .value("target", TestValue::Reference(0x7533));
    if root_value {
        owner = owner.field("DamagePhysical", TestType::Single);
        record = record.value("DamagePhysical", TestValue::Single(30.0));
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Leaf").field("DamagePhysical", TestType::Single))
        .structure(TestStruct::new("External").field("ExternalOnly", TestType::UInt32))
        .structure(owner)
        .record(record)
        .record(
            TestRecord::new(0x7533, "External", "External.One", "external.xml")
                .value("ExternalOnly", TestValue::UInt32(99)),
        )
        .build()
}
