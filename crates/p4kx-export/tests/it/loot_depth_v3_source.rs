// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, RecordTree, Value};

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::loot_depth_v3_fixture as fixture;

pub(crate) struct Materialized {
    pub(crate) nodes: usize,
    pub(crate) values: usize,
}

pub(crate) fn check(input: &Fixture, requested: usize) -> TestResult<Materialized> {
    assert!(
        input.entry.origin().matches_archive(&input.archive),
        "actual original Archive binding"
    );
    let forge = input.entry.open()?;
    assert_eq!(
        forge.records().len(),
        1,
        "one actual original physical Loot Record"
    );
    let structure = forge
        .structs()
        .find(|value| value.name().is_ok_and(|name| name == "DepthNode"))
        .ok_or("actual stored DepthNode mapping")?;
    assert_eq!(
        structure.instance_count(),
        u32::try_from(requested)?,
        "all requested original instances exist independently of the canonical read policy"
    );
    let record = forge.record(0).ok_or("original depth Record")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual original canonical owner");
    assert_eq!(record.index(), 0, "actual physical owner position");
    assert_eq!(
        record.guid().to_string(),
        fixture::GUID,
        "actual source identity"
    );
    root_declaration(&tree)?;
    let nodes = requested;
    let tail = chain(&tree, nodes)?;
    check_tail(&tree, tail);
    let values = nodes
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or("synthetic original occurrence count overflow")?;
    Ok(Materialized { nodes, values })
}

fn root_declaration(tree: &RecordTree<'_>) -> TestResult {
    assert_eq!(tree.fields().len(), 1, "one original root field");
    let field = tree.fields().first().ok_or("original filter field")?;
    assert_eq!(field.name(), "filter", "actual root declaration");
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "actual owned pointer depth"
    );
    assert!(
        !field.storage().is_array(),
        "the chain is not counted array nesting"
    );
    Ok(())
}

fn check_tail(tree: &RecordTree<'_>, tail: &Value<'_>) {
    assert_eq!(
        tail,
        &Value::Null,
        "the complete original chain terminates in a real null"
    );
    assert_eq!(tree.error_count(), 0, "no original source failure");
}
fn chain<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    count: usize,
) -> TestResult<&'tree Value<'source>> {
    let mut value = tree.field("filter").ok_or("actual original chain root")?;
    for ordinal in 0..count {
        let Value::Struct(node) = value else {
            return Err("each expected original owned node was actually decoded".into());
        };
        assert_eq!(node.type_name(), "DepthNode", "actual original nested type");
        assert_eq!(
            node.fields().len(),
            2,
            "one marker and one next edge per original instance"
        );
        for (field, (name, data_type)) in node.fields().iter().zip([
            ("marker", DataType::UInt32),
            ("next", DataType::StrongPointer),
        ]) {
            assert_eq!(field.name(), name, "original nested declaration order");
            assert_eq!(
                field.storage().data_type(),
                data_type,
                "actual original nested storage"
            );
            assert!(
                !field.storage().is_array(),
                "these are original scalar fields"
            );
        }
        assert_eq!(
            node.field("marker"),
            Some(&Value::UInt32(u32::try_from(ordinal)?)),
            "every decoded original value, including zero, precedes export assertions"
        );
        value = node.field("next").ok_or("actual original next pointer")?;
    }
    Ok(value)
}
