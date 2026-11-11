// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Guid, Record, RecordTree, StructValue, Value};

use super::fixture::{LEAF, NODE, POOL, ROOT};
use super::support::{self, TestResult};

pub(super) fn original<'source>(
    forge: &'source DataForge<'source>,
    depth: u32,
) -> TestResult<Record<'source>> {
    schema(forge, depth)?;
    let record = forge.record(0).ok_or("original reward owner")?;
    assert_eq!(
        record.guid(),
        Guid::from_stored([0x11; 16]),
        "the owner GUID is genuinely selected in pools"
    );
    assert_eq!(record.type_name()?, ROOT, "the actual original owner type");
    assert_eq!(
        record.name()?,
        "RewardRoot.Source",
        "the actual original owner name"
    );
    assert_eq!(
        record.record_path()?,
        "rewards/deep",
        "the actual original owner path"
    );
    Ok(record)
}

pub(super) fn complete<'source>(
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    depth: u32,
) -> TestResult {
    support::clean(tree, record);
    let root = tree.root();
    support::fields(
        root,
        &[
            "debugName",
            "chain",
            "after",
            "self",
            "rootWeak",
            "local",
            "ignore",
        ],
    );
    assert_eq!(
        root.field("debugName"),
        Some(&Value::String("root")),
        "the exact original scope name"
    );
    support::declaration(root, "debugName", DataType::String, false, None)?;
    support::declaration(root, "chain", DataType::StrongPointer, false, Some(0))?;
    support::declaration(root, "after", DataType::StrongPointer, false, Some(1))?;
    chain(forge, root, depth)?;
    support::leaf(
        forge,
        support::structure(support::value(root, "after")?)?,
        [Some(2), None, None],
        0.75,
    )?;
    counters(forge, record, root, depth)?;
    Ok(())
}

fn schema(forge: &DataForge<'_>, depth: u32) -> TestResult {
    assert_eq!(
        forge.records().len(),
        4,
        "the owner and all three genuine pool targets exist"
    );
    let expected = [(NODE, depth), (LEAF, 2), (ROOT, 1), (POOL, 3)];
    assert_eq!(
        forge.structs().len(),
        expected.len(),
        "all original declarations are present"
    );
    for (structure, (name, instances)) in forge.structs().zip(expected) {
        assert_eq!(structure.name()?, name, "original schema declaration order");
        assert_eq!(
            structure.instance_count(),
            instances,
            "every named original instance exists"
        );
        assert!(
            structure.parent().is_none(),
            "the source has no inherited hidden slots"
        );
    }
    let survey = forge.survey()?;
    assert_eq!(
        survey.unreadable_structs, 0,
        "every actual source layout is readable"
    );
    assert_eq!(
        survey.pointers.narrow,
        u64::from(depth) + 4,
        "all original owned and weak pointers target real instances"
    );
    assert_eq!(
        survey.pointers.null,
        u64::from(depth),
        "only the terminal next and unused node rewards are null"
    );
    assert_eq!(
        (survey.pointers.wide, survey.pointers.dangling),
        (0, 0),
        "no pointer target is missing or misencoded"
    );
    Ok(())
}

fn chain<'source>(
    forge: &'source DataForge<'source>,
    root: &StructValue<'source>,
    depth: u32,
) -> TestResult {
    let mut current = support::value(root, "chain")?;
    for index in 0..depth {
        let node = support::structure(current)?;
        assert_eq!(
            node.type_name(),
            NODE,
            "every original Strong target keeps its concrete type"
        );
        support::fields(node, &["mark", "next", "reward"]);
        assert_eq!(
            node.field("mark"),
            Some(&Value::UInt32(index + 1)),
            "every original node and its position survive before reward collection"
        );
        support::declaration(node, "mark", DataType::UInt32, false, None)?;
        support::declaration(node, "next", DataType::StrongPointer, false, Some(0))?;
        support::declaration(node, "reward", DataType::StrongPointer, false, Some(1))?;
        let reward = support::value(node, "reward")?;
        if index + 1 == depth {
            support::leaf(
                forge,
                support::structure(reward)?,
                [Some(1), Some(1), Some(2)],
                0.25,
            )?;
        } else {
            assert_eq!(
                reward,
                &Value::Null,
                "no intermediate node hides another reward"
            );
        }
        current = support::value(node, "next")?;
    }
    assert_eq!(
        current,
        &Value::Null,
        "the only chain terminator is the actual original null"
    );
    Ok(())
}

fn counters<'source>(
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    root: &StructValue<'source>,
    depth: u32,
) -> TestResult {
    support::declaration(root, "self", DataType::Reference, false, Some(2))?;
    support::declaration(root, "rootWeak", DataType::WeakPointer, false, Some(2))?;
    support::declaration(root, "local", DataType::WeakPointer, false, Some(0))?;
    support::declaration(root, "ignore", DataType::Reference, false, Some(3))?;
    support::target(forge, support::value(root, "self")?, 0)?;
    support::target(forge, support::value(root, "rootWeak")?, 0)?;
    support::target(forge, support::value(root, "ignore")?, 3)?;
    local_weak(record, root, depth)
}

fn local_weak(record: &Record<'_>, root: &StructValue<'_>, depth: u32) -> TestResult {
    let Value::Reference(local) = support::value(root, "local")? else {
        return Err("the actual local Weak target is missing".into());
    };
    assert_eq!(
        local.guid(),
        record.guid(),
        "a local Weak target belongs to the original owner"
    );
    assert_eq!(
        local.record_index(),
        record.index(),
        "the original local owner position"
    );
    assert_eq!(
        local.record_path(),
        record.record_path()?,
        "the actual local owner path"
    );
    assert_eq!(
        local.type_name(),
        NODE,
        "the real local Weak points to a Node instance"
    );
    assert!(
        local.location().is_some(),
        "the local Weak is distinct from a root Record Reference"
    );
    let receipt = local.original_target();
    assert!(
        receipt.shares_source_with(record),
        "the local admission retains its exact original Forge"
    );
    let mut positions = vec![1];
    positions.extend(std::iter::repeat_n(1, usize::try_from(depth - 1)?));
    assert_eq!(
        receipt.instance_positions(),
        Some(positions.as_slice()),
        "every actual root and next-field ordinal identifies the original final Node"
    );
    Ok(())
}
