// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_dataforge::{DataForge, TagBuildEvent, TagPathEdgeKind, TagTree, Value};
use p4kx_testkit::{TestRecord, TestValue};

use super::{
    PATH, TestResult, builder, forge_guid, guid_of, original_name, original_roots, parent,
};

const LEAVES: usize = 65_535;
const FIRST_LEAF: usize = 3;
const LAST_LEAF: usize = FIRST_LEAF + LEAVES - 1;

#[test]
fn valid_distinct_leaves_survive_more_than_sixty_four_mib_of_joined_paths() -> TestResult {
    let name = "x".repeat(256);
    let leaf_name = "y".repeat(253);
    let bytes = bytes(&name, &leaf_name);
    let forge = DataForge::open(&bytes)?;
    original(&forge, &name, &leaf_name)?;
    let path = expected_path(&name, &leaf_name);

    let leaf_record = u32::try_from(LAST_LEAF + 1)?;
    let mut observed = None;
    let tree = TagTree::build_observed(&forge, &|| false, &mut |event| {
        if let TagBuildEvent::PathChosen {
            record,
            parent_position,
            incoming,
            path,
            depth,
            ..
        } = event
            && record.index() == leaf_record
        {
            observed = Some((
                parent_position,
                incoming.map(|edge| (edge.record().index(), edge.kind(), edge.ordinal())),
                path.byte_len(),
                u64::from(depth),
            ));
        }
        Ok::<_, Infallible>(())
    })?
    .ok_or("tag build cancelled")?;
    assert_eq!(
        tree.len(),
        FIRST_LEAF + LEAVES,
        "the aggregate of valid paths cannot discard the final leaf"
    );
    assert_eq!(tree.issues().unlisted(), 0);
    assert_eq!(
        observed,
        Some((Some(2), Some((3, TagPathEdgeKind::Child, LEAVES)), 1024, 3)),
        "the final path keeps its actual physical parent and stored slot"
    );
    let leaf = tree
        .get(forge_guid(LAST_LEAF)?)
        .ok_or("the last original leaf")?;
    assert_eq!(leaf.path(), path);
    assert_eq!(leaf.parent().map(|tag| tag.position()), Some(2));
    Ok(())
}

fn expected_path(name: &str, leaf_name: &str) -> String {
    let path = [name, name, name, leaf_name].join(".");
    assert_eq!(
        path.len(),
        1024,
        "every leaf fits the original individual path cutoff"
    );
    let prefix_bytes: usize = (1..=FIRST_LEAF)
        .map(|parts| parts * name.len() + parts - 1)
        .sum();
    assert_eq!(
        prefix_bytes + (LEAVES - 1) * path.len(),
        (64 << 20) - 509,
        "all earlier valid paths fit the old aggregate cutoff"
    );
    assert_eq!(
        prefix_bytes + LEAVES * path.len(),
        (64 << 20) + 515,
        "only the final leaf crosses the old aggregate cutoff"
    );
    path
}

fn bytes(name: &str, leaf: &str) -> Vec<u8> {
    let mut children = Vec::with_capacity(LEAVES + 1);
    children.push(TestValue::Null);
    children.extend((FIRST_LEAF..=LAST_LEAF).map(|index| TestValue::Reference(guid_of(index))));
    let mut database = builder(&[guid_of(0)])
        .record(parent(0, name, 1))
        .record(parent(1, name, 2))
        .record(tag(2, "Tag.Fanout", name, TestValue::Array(children)));
    for index in FIRST_LEAF..=LAST_LEAF {
        database = database.record(tag(index, "Tag.Leaf", leaf, TestValue::Array(Vec::new())));
    }
    database.build()
}

fn tag(index: usize, record_name: &str, name: &str, children: TestValue) -> TestRecord {
    TestRecord::new(guid_of(index), "Tag", record_name, PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(1))
        .value("children", children)
}

fn original(forge: &DataForge<'_>, name: &str, leaf: &str) -> TestResult {
    assert_eq!(
        forge.records().count(),
        LEAVES + FIRST_LEAF + 1,
        "all original physical Records"
    );
    original_roots(forge, &[0])?;
    for index in 0..FIRST_LEAF {
        original_name(forge, index, name)?;
    }
    super::original_child(forge, 0, 1)?;
    super::original_child(forge, 1, 2)?;
    original_name(forge, FIRST_LEAF, leaf)?;
    original_name(forge, LAST_LEAF, leaf)?;
    let parent = forge.record(3).ok_or("the actual fanout Record")?;
    let tree = parent.tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "all original fanout references resolve"
    );
    let Some(Value::Array(items)) = tree.field("children") else {
        return Err("the original wide Reference array".into());
    };
    assert_eq!(
        items.len(),
        LEAVES + 1,
        "every original child occurrence including the leading null"
    );
    assert!(
        matches!(items.first(), Some(Value::Null)),
        "the original null slot precedes every real leaf"
    );
    for (item, index) in items.iter().skip(1).zip(FIRST_LEAF..=LAST_LEAF) {
        assert!(
            matches!(item, Value::Reference(reference) if usize::try_from(reference.record_index()).ok() == Some(index + 1)),
            "every original child points to its separate physical Record"
        );
    }
    Ok(())
}
