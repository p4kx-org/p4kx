// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, RecordTree, StructValue, Value};

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::template_flow_depth_fixture::{self as fixture, Branch, Tail};

pub(crate) fn check(input: &Fixture, branch: Branch, count: usize, tail: Tail) -> TestResult {
    check_owner(
        input,
        branch,
        count,
        tail,
        "ContractTemplate",
        fixture::GUID,
    )
}

pub(crate) fn check_owner(
    input: &Fixture,
    branch: Branch,
    count: usize,
    tail: Tail,
    owner_type: &str,
    guid: &str,
) -> TestResult {
    with_owner_tree(input, owner_type, guid, |tree| {
        let mut next = condition(tree)?;
        for _ in 0..count {
            let node = structure(next)?;
            assert_eq!(
                node.type_name(),
                branch.source_type(),
                "every original chain occurrence"
            );
            next = child(node, branch)?;
        }
        check_tail(structure(next)?, tail);
        Ok(())
    })
}

fn with_tree(input: &Fixture, check: impl FnOnce(&RecordTree<'_>) -> TestResult) -> TestResult {
    with_owner_tree(input, "ContractTemplate", fixture::GUID, check)
}

fn with_owner_tree(
    input: &Fixture,
    owner_type: &str,
    guid: &str,
    check: impl FnOnce(&RecordTree<'_>) -> TestResult,
) -> TestResult {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual original flow template")?;
    assert_eq!(
        record.guid().to_string(),
        guid,
        "actual original owner GUID"
    );
    assert_eq!(record.type_name()?, owner_type, "actual source family");
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "paired actual original tree");
    assert_eq!(
        tree.error_count(),
        0,
        "all original conditions are readable before export"
    );
    check(&tree)
}

fn condition<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> TestResult<&'tree Value<'source>> {
    let flow = structure(tree.field("missionFlow").ok_or("actual original flow")?)?;
    assert_eq!(flow.type_name(), "MissionFlow", "actual flow type");
    let Some(Value::Array(triggers)) = flow.field("triggers") else {
        return Err("actual trigger array".into());
    };
    assert_eq!(triggers.len(), 1, "one actual original trigger");
    let trigger = structure(triggers.first().ok_or("actual trigger")?)?;
    let first = trigger
        .fields()
        .first()
        .ok_or("actual trigger declaration")?;
    assert_eq!(first.name(), "condition", "actual declared trigger field");
    assert_eq!(
        first.storage().data_type(),
        DataType::StrongPointer,
        "actual strong pool input"
    );
    assert!(!first.storage().is_array(), "trigger condition is scalar");
    Ok(first.value())
}

fn child<'tree, 'source>(
    node: &'tree StructValue<'source>,
    branch: Branch,
) -> TestResult<&'tree Value<'source>> {
    assert_eq!(node.fields().len(), 1, "one original child declaration");
    let field = node.fields().first().ok_or("actual child declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "real pointer pool storage"
    );
    match branch {
        Branch::Not => {
            assert_eq!(field.name(), "condition", "actual scalar child field");
            assert!(!field.storage().is_array(), "original NOT is scalar");
            Ok(field.value())
        }
        Branch::And | Branch::Or => {
            assert_eq!(field.name(), "conditions", "actual ordered child array");
            assert!(field.storage().is_array(), "original AND/OR is an array");
            let Value::Array(values) = field.value() else {
                return Err("readable child array".into());
            };
            assert_eq!(
                values.len(),
                1,
                "one real child in each original chain array"
            );
            values.first().ok_or_else(|| "actual sole child".into())
        }
    }
}

fn check_tail(node: &StructValue<'_>, tail: Tail) {
    match tail {
        Tail::Known => {
            assert_eq!(
                node.type_name(),
                "MissionFlowCondition_OnMissionStart",
                "real known leaf"
            );
            assert!(node.fields().is_empty(), "actual leaf has no fields");
        }
        Tail::Unknown => {
            assert_eq!(
                node.type_name(),
                "MissionFlowCondition_Mystery",
                "readable genuine unknown leaf"
            );
            assert!(
                node.fields().is_empty(),
                "unknown is readable, not a source error"
            );
        }
        Tail::Missing => {
            assert_eq!(
                node.type_name(),
                "MissionFlowCondition_NOT",
                "real NOT leaf"
            );
            assert_eq!(
                node.field("condition"),
                Some(&Value::Null),
                "actual missing child is null"
            );
        }
        Tail::Empty => {
            assert_eq!(
                node.type_name(),
                "MissionFlowCondition_AND",
                "real AND leaf"
            );
            assert!(
                matches!(node.field("conditions"), Some(Value::Array(values)) if values.is_empty()),
                "actual empty array"
            );
        }
    }
}

fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(node) => Ok(node),
        _ => Err("actual original condition struct".into()),
    }
}

pub(crate) fn mixed(input: &Fixture) -> TestResult {
    with_tree(input, |tree| {
        let root = structure(condition(tree)?)?;
        assert_eq!(
            root.type_name(),
            "MissionFlowCondition_AND",
            "actual ordered root"
        );
        let values = children(root)?;
        assert_eq!(values.len(), 2, "both actual root occurrences");
        check_tail(
            structure(values.first().ok_or("first original child")?)?,
            Tail::Known,
        );
        let alternatives = structure(values.get(1).ok_or("second original child")?)?;
        assert_eq!(
            alternatives.type_name(),
            "MissionFlowCondition_OR",
            "actual nested alternation"
        );
        let choices = children(alternatives)?;
        assert_eq!(choices.len(), 2, "both actual distinct alternatives");
        check_tail(
            structure(choices.first().ok_or("actual null NOT")?)?,
            Tail::Missing,
        );
        check_tail(
            structure(choices.get(1).ok_or("actual empty AND")?)?,
            Tail::Empty,
        );
        Ok(())
    })
}

fn children<'tree, 'source>(
    node: &'tree StructValue<'source>,
) -> TestResult<&'tree [Value<'source>]> {
    let field = node.fields().first().ok_or("original array declaration")?;
    assert_eq!(node.fields().len(), 1, "one actual original array field");
    assert_eq!(field.name(), "conditions", "actual child array name");
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "original array pool storage"
    );
    assert!(field.storage().is_array(), "original array declaration");
    match field.value() {
        Value::Array(values) => Ok(values),
        _ => Err("actual readable original child array".into()),
    }
}
