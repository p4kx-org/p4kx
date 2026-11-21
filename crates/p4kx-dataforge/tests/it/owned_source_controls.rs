// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, Field, Record, RecordTree, TreeBuilder, Value};

use crate::owned_source_fixture::{HOLDER, NODE, TestResult};

pub(crate) fn original<'source>(
    forge: &'source DataForge<'source>,
    branches: u32,
    depth: u32,
) -> TestResult<Record<'source>> {
    assert_eq!(
        forge.structs().len(),
        2,
        "the actual original source has exactly two declarations"
    );
    owner_schema(forge)?;
    node_schema(forge, branches, depth)?;
    let survey = forge.survey()?;
    assert_eq!(
        survey.unreadable_structs, 0,
        "independent source layouts are readable"
    );
    assert_eq!(
        survey.pointers.narrow,
        u64::from(branches) * u64::from(depth) + 1,
        "root pool, all nonterminal next pointers, and the final weak pointer are actual valid targets"
    );
    assert_eq!(
        survey.pointers.null,
        u64::from(branches),
        "one real terminal null per branch"
    );
    assert_eq!(
        (survey.pointers.wide, survey.pointers.dangling),
        (0, 0),
        "these source pointers have no missing or wide targets"
    );
    let record = forge.record(0).ok_or("original owner")?;
    assert_eq!(forge.records().len(), 1, "one physical owner");
    assert_eq!(record.type_name()?, HOLDER, "original owner type");
    assert_eq!(
        record.record_path()?,
        "source/owned",
        "original owner filename projection"
    );
    let probe = forge
        .pointer_probe("roots", NODE)
        .ok_or("original Node pointer probe")?;
    assert!(
        TreeBuilder::new().holds(&probe, &record)?,
        "the original root array actually points to existing Nodes before tree materialization"
    );
    Ok(record)
}

fn node_schema(forge: &DataForge<'_>, branches: u32, depth: u32) -> TestResult {
    let node = forge
        .structs()
        .find(|value| value.name().is_ok_and(|name| name == NODE))
        .ok_or("original Node declaration")?;
    assert_eq!(node.index(), 0, "the actual original Node struct position");
    assert!(
        node.parent().is_none(),
        "the two original fields have no hidden inherited prefix"
    );
    assert_eq!(
        node.size(),
        12,
        "the original UInt32 and pointer storage widths"
    );
    assert_eq!(
        node.instance_count(),
        branches * depth,
        "every named original instance exists"
    );
    let fields: Vec<_> = node.properties().collect();
    assert_eq!(fields.len(), 2, "the actual original two declarations");
    let mark = fields.first().ok_or("original mark declaration")?;
    assert_eq!(mark.name()?, "mark", "first original declaration");
    assert_eq!(mark.data_type()?, DataType::UInt32, "original mark storage");
    let next = fields.get(1).ok_or("original next declaration")?;
    assert_eq!(next.name()?, "next", "second original declaration");
    assert_eq!(
        next.data_type()?,
        DataType::StrongPointer,
        "original owned edge"
    );
    assert!(!next.is_array(), "the original chain is a scalar pointer");
    assert_eq!(
        next.target_struct().ok_or("original target")?.index(),
        0,
        "the declaration targets the original Node class"
    );
    Ok(())
}

fn owner_schema(forge: &DataForge<'_>) -> TestResult {
    let holder = forge
        .structs()
        .find(|value| value.name().is_ok_and(|name| name == HOLDER))
        .ok_or("original Holder declaration")?;
    assert_eq!(holder.index(), 1, "the original owner declaration position");
    assert_eq!(holder.instance_count(), 1, "one original owner instance");
    assert!(
        holder.parent().is_none(),
        "the owner has no inherited fields"
    );
    let fields: Vec<_> = holder.properties().collect();
    assert_eq!(fields.len(), 2, "the two original owner fields");
    let roots = fields.first().ok_or("original root-array declaration")?;
    assert_eq!(roots.name()?, "roots", "original owner field zero");
    assert_eq!(
        roots.data_type()?,
        DataType::StrongPointer,
        "original owned array slots"
    );
    assert!(roots.is_array(), "the source roots declaration is an array");
    let tail = fields.get(1).ok_or("original weak declaration")?;
    assert_eq!(tail.name()?, "tail", "original owner field one");
    assert_eq!(
        tail.data_type()?,
        DataType::WeakPointer,
        "the final link is an original weak pointer"
    );
    assert!(!tail.is_array(), "one original weak slot");
    assert_eq!(
        tail.target_struct().ok_or("original weak target")?.index(),
        0,
        "the weak declaration actually targets Node"
    );
    Ok(())
}

pub(crate) fn complete(tree: &RecordTree<'_>, branches: u32, depth: u32) -> TestResult {
    let Some(Value::Array(roots)) = tree.field("roots") else {
        return Err("the original root array must remain an array".into());
    };
    assert_eq!(
        roots.len(),
        usize::try_from(branches)?,
        "every original root slot survives"
    );
    for (branch, root) in roots.iter().enumerate() {
        branch_values(root, u32::try_from(branch)?, depth)?;
    }
    assert_eq!(
        tree.error_count(),
        0,
        "valid original owned sources have no invented error values"
    );
    assert_eq!(
        tree.located_errors().count(),
        0,
        "located errors agree with the complete values"
    );
    Ok(())
}

fn branch_values(root: &Value<'_>, branch: u32, depth: u32) -> TestResult {
    let mut current = root;
    for node in 0..depth {
        let Value::Struct(structure) = current else {
            return Err(format!("missing original owned node {branch}/{node}: {current:?}").into());
        };
        assert_eq!(
            structure.type_name(),
            NODE,
            "the actual target struct survives"
        );
        assert_eq!(
            structure
                .fields()
                .iter()
                .map(Field::name)
                .collect::<Vec<_>>(),
            ["mark", "next"],
            "every original declaration stays in order"
        );
        assert_eq!(
            structure.field("mark"),
            Some(&Value::UInt32(branch * depth + node + 1)),
            "every original scalar and its branch position survive"
        );
        current = structure.field("next").ok_or("the original next field")?;
    }
    assert_eq!(
        current,
        &Value::Null,
        "only the actual terminal null ends a branch"
    );
    Ok(())
}

pub(crate) fn tail(
    tree: &RecordTree<'_>,
    owner: &Record<'_>,
    branches: u32,
    depth: u32,
) -> TestResult {
    let Some(Value::Reference(reference)) = tree.field("tail") else {
        return Err("the genuine weak pointer retains its original instance target".into());
    };
    let mut expected = vec![0_u64, u64::from(branches - 1)];
    expected.extend(std::iter::repeat_n(1, usize::try_from(depth - 1)?));
    let receipt = reference.original_target();
    assert!(
        receipt.shares_source_with(owner),
        "the real target receipt belongs to the original Forge"
    );
    assert_eq!(
        receipt.instance_positions(),
        Some(expected.as_slice()),
        "original field and array ordinals survive every nesting level"
    );
    assert_eq!(
        reference.type_name(),
        NODE,
        "the weak target retains its actual class"
    );
    Ok(())
}
