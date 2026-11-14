// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::error::Error;

use p4kx_dataforge::{DataForge, TagBuildEvent, TagPathEdgeKind, TagTree, Value};
use p4kx_testkit::{TestRecord, TestValue};

use super::{PATH, builder, forge_guid, guid_of, tag};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type Edge = (u32, TagPathEdgeKind, usize);

#[derive(Debug)]
struct Chosen {
    record: u32,
    parent: Option<u32>,
    incoming: Option<Edge>,
    path: String,
    depth: u64,
    listed: bool,
}

#[test]
fn the_complete_utf8_name_selects_its_child_before_the_later_root_occurrence() -> TestResult {
    let boundary = "é".repeat(128);
    let longer = format!("{boundary}x");
    assert_eq!(
        boundary.len(),
        256,
        "the earlier root is at the old byte boundary"
    );
    assert_eq!(
        longer.len(),
        257,
        "the next genuine name exceeds the old byte boundary"
    );
    let bytes = builder(&[guid_of(0), guid_of(1), guid_of(2)])
        .record(tag(guid_of(0), &boundary, &[]))
        .record(parent(1, &longer, 2))
        .record(tag(guid_of(2), "尾", &[]))
        .build();
    let forge = DataForge::open(&bytes)?;
    original_roots(&forge, &[0, 1, 2])?;
    original_name(&forge, 0, &boundary)?;
    original_name(&forge, 1, &longer)?;
    original_name(&forge, 2, "尾")?;
    original_child(&forge, 1, 2)?;

    let (tree, chosen) = observe(&forge)?;
    assert_eq!(
        tree.len(),
        3,
        "every readable Tag must have its selected physical path"
    );
    assert_eq!(tree.issues().unreadable(), 0, "the UTF-8 source reads");
    assert_eq!(
        tree.issues().repeated(),
        1,
        "the later authored root occurrence is still counted"
    );
    let root_positions: Vec<_> = tree.roots().map(|tag| tag.position()).collect();
    assert_eq!(
        root_positions,
        [0, 1],
        "the earlier physical child wins over its later root occurrence"
    );
    let child = chosen.get(2).ok_or("the actual selected child")?;
    assert_eq!(child.record, 3, "the physical child Record");
    assert_eq!(
        child.parent,
        Some(1),
        "the selected parent's preorder position"
    );
    assert_eq!(
        child.incoming,
        Some((2, TagPathEdgeKind::Child, 1)),
        "original child slot after null"
    );
    assert_eq!(
        child.path,
        format!("{longer}.尾"),
        "the exact full UTF-8 path"
    );
    Ok(())
}

#[test]
fn a_valid_path_can_pass_1024_bytes_through_an_authored_empty_name() -> TestResult {
    let wide = "p".repeat(256);
    let edge = "q".repeat(253);
    let names = [wide.as_str(), &wide, &wide, &edge, "", "Tail"];
    let bytes = chain(&names);
    let forge = DataForge::open(&bytes)?;
    original_chain(&forge, &names)?;

    let (tree, chosen) = observe(&forge)?;
    assert_eq!(
        tree.len(),
        names.len(),
        "no readable child is lost at the old path byte boundary"
    );
    assert_eq!(
        tree.issues().unlisted(),
        0,
        "all nodes remain below the listed root"
    );
    assert_eq!(
        chosen.get(3).ok_or("the exact boundary path")?.path.len(),
        1024
    );
    let empty = chosen.get(4).ok_or("the authored empty-name node")?;
    assert_eq!(
        empty.path,
        names
            .get(..5)
            .ok_or("the original five-name prefix")?
            .join("."),
        "the empty name still contributes a separator"
    );
    assert_eq!(empty.path.len(), 1025);
    assert_eq!(empty.incoming, Some((4, TagPathEdgeKind::Child, 1)));
    assert_eq!(tree.at(4).ok_or("the physical empty-name tag")?.name(), "");
    let tail = tree.at(5).ok_or("the actual selected descendant")?;
    assert_eq!(tail.path(), names.join("."));
    assert_eq!(tail.parent().map(|tag| tag.position()), Some(4));
    Ok(())
}

#[test]
fn a_readable_short_name_chain_retains_the_first_slot_beyond_sixty_four_levels() -> TestResult {
    let names = ["x"; 66];
    let bytes = chain(&names);
    let forge = DataForge::open(&bytes)?;
    original_chain(&forge, &names)?;

    let (tree, chosen) = observe(&forge)?;
    assert_eq!(
        tree.len(),
        names.len(),
        "every physical tag in the valid chain is selected"
    );
    assert_eq!(
        tree.issues().unlisted(),
        0,
        "the deepest node remains below the listed root"
    );
    let root = tree.roots().next().ok_or("the actual declared root")?;
    assert_eq!(
        root.descendants().count(),
        65,
        "the exact exclusive preorder interval"
    );
    let past = chosen
        .get(64)
        .ok_or("the first node beyond the old depth limit")?;
    assert_eq!(past.record, 65);
    assert_eq!(past.parent, Some(63));
    assert_eq!(past.incoming, Some((64, TagPathEdgeKind::Child, 1)));
    assert_eq!(past.depth, 64);
    assert!(past.listed, "the original listing status survives");
    let tail = tree.at(65).ok_or("the actual deepest node")?;
    assert_eq!(tail.depth(), 65);
    assert_eq!(tail.path(), names.join("."));
    Ok(())
}

fn parent(index: usize, name: &str, child: usize) -> TestRecord {
    TestRecord::new(guid_of(index), "Tag", format!("Tag.{index}"), PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(1))
        .value(
            "children",
            TestValue::Array(vec![TestValue::Null, TestValue::Reference(guid_of(child))]),
        )
}

fn chain(names: &[&str]) -> Vec<u8> {
    let mut database = builder(&[guid_of(0)]);
    for (index, name) in names.iter().enumerate() {
        let record = if index + 1 < names.len() {
            parent(index, name, index + 1)
        } else {
            tag(guid_of(index), name, &[])
        };
        database = database.record(record);
    }
    database.build()
}

fn original_chain(forge: &DataForge<'_>, names: &[&str]) -> TestResult {
    assert_eq!(
        forge.records().count(),
        names.len() + 1,
        "one physical database plus every original Tag"
    );
    original_roots(forge, &[0])?;
    for (index, name) in names.iter().enumerate() {
        original_name(forge, index, name)?;
        if index + 1 < names.len() {
            original_child(forge, index, index + 1)?;
        }
    }
    Ok(())
}

fn original_roots(forge: &DataForge<'_>, expected: &[usize]) -> TestResult {
    let database = forge.record(0).ok_or("original database Record")?;
    assert_eq!(
        database.type_name()?,
        "TagDatabase",
        "the roots come from the actual TagDatabase"
    );
    let tree = database.tree()?;
    let Some(Value::Array(items)) = tree.field("tags") else {
        return Err("original database relation array".into());
    };
    assert_eq!(
        items.len(),
        expected.len(),
        "every original database root slot"
    );
    for (item, index) in items.iter().zip(expected) {
        original_reference(item, *index)?;
    }
    Ok(())
}

fn original_name(forge: &DataForge<'_>, index: usize, name: &str) -> TestResult {
    let record = forge
        .record(u32::try_from(index + 1)?)
        .ok_or("original Tag Record")?;
    assert_eq!(
        record.guid(),
        forge_guid(index)?,
        "the selected Record retains its original identity"
    );
    assert_eq!(
        record.type_name()?,
        "Tag",
        "the selected Record is an original Tag"
    );
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), 0, "the original Tag is readable");
    assert!(
        matches!(tree.field("tagName"), Some(Value::String(value)) if *value == name),
        "the original Tag has its exact authored name"
    );
    Ok(())
}

fn original_child(forge: &DataForge<'_>, parent: usize, child: usize) -> TestResult {
    let record = forge
        .record(u32::try_from(parent + 1)?)
        .ok_or("original parent Record")?;
    let tree = record.tree()?;
    let Some(Value::Array(items)) = tree.field("children") else {
        return Err("original child relation array".into());
    };
    let [first, selected] = items.as_ref() else {
        return Err("two actual child slots".into());
    };
    assert!(
        matches!(first, Value::Null),
        "the stored null occupies slot zero"
    );
    original_reference(selected, child)
}

fn original_reference(value: &Value<'_>, index: usize) -> TestResult {
    let Value::Reference(reference) = value else {
        return Err("original resolved Tag Reference".into());
    };
    assert_eq!(
        reference.guid(),
        forge_guid(index)?,
        "the authored relation targets the exact original Tag"
    );
    assert_eq!(
        reference.record_index(),
        u32::try_from(index + 1)?,
        "actual physical target"
    );
    Ok(())
}

fn observe(forge: &DataForge<'_>) -> TestResult<(TagTree, Vec<Chosen>)> {
    let mut chosen = Vec::new();
    let tree = TagTree::build_observed(forge, &|| false, &mut |event| {
        if let TagBuildEvent::PathChosen {
            record,
            position,
            parent_position,
            incoming,
            path,
            depth,
            listed,
        } = event
        {
            assert_eq!(
                usize::try_from(position).ok(),
                Some(chosen.len()),
                "actual selected preorder"
            );
            chosen.push(Chosen {
                record: record.index(),
                parent: parent_position,
                incoming: incoming.map(|edge| (edge.record().index(), edge.kind(), edge.ordinal())),
                path: path.to_string(),
                depth: u64::from(depth),
                listed,
            });
        }
        Ok::<_, Infallible>(())
    })?
    .ok_or("tag build cancelled")?;
    Ok((tree, chosen))
}

#[path = "tag_default_aggregate_source.rs"]
mod aggregate;
