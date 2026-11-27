// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Field, Guid, Record, Reference, StructValue, Value};

use super::fixture::{LEAF, POOL};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn value<'tree, 'source>(
    node: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<&'tree Value<'source>> {
    node.field(name)
        .ok_or_else(|| format!("original field {name} is missing").into())
}

pub(super) fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(node) => Ok(node),
        _ => Err("original owned struct is missing".into()),
    }
}

pub(super) fn array<'tree, 'source>(
    node: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<&'tree [Value<'source>]> {
    match value(node, name)? {
        Value::Array(items) => Ok(items),
        _ => Err("original array is missing".into()),
    }
}

pub(super) fn fields(node: &StructValue<'_>, expected: &[&str]) {
    assert_eq!(
        node.fields().iter().map(Field::name).collect::<Vec<_>>(),
        expected,
        "every original declaration remains in stored order"
    );
}

pub(super) fn declaration(
    node: &StructValue<'_>,
    name: &str,
    data_type: DataType,
    array: bool,
    target: Option<u16>,
) -> TestResult {
    let field = node
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .ok_or("original storage declaration")?;
    let storage = field.storage();
    assert_eq!(
        storage.data_type(),
        data_type,
        "the actual original decoder storage"
    );
    assert_eq!(storage.is_array(), array, "the original scalar/array shape");
    if let Some(target) = target {
        assert_eq!(
            storage.target(),
            target,
            "the actual original target declaration"
        );
    }
    Ok(())
}

pub(super) fn target<'tree, 'source>(
    forge: &'source DataForge<'source>,
    value: &'tree Value<'source>,
    index: u32,
) -> TestResult<&'tree Reference<'source>> {
    let Value::Reference(reference) = value else {
        return Err("the actual original Reference is missing".into());
    };
    let target = original_record(forge, index)?;
    assert_eq!(
        reference.guid(),
        target.guid(),
        "the actual original target GUID"
    );
    assert_eq!(
        reference.record_index(),
        target.index(),
        "the original target position"
    );
    assert_eq!(
        reference.record_path(),
        target.record_path()?,
        "the original target path"
    );
    assert_eq!(
        reference.type_name(),
        target.type_name()?,
        "the original target type"
    );
    assert!(
        reference.location().is_none(),
        "a genuine Record target has no instance location"
    );
    assert!(
        reference.original_target().shares_source_with(&target),
        "the original admitted target belongs to this exact Forge"
    );
    Ok(reference)
}

fn original_record<'source>(
    forge: &'source DataForge<'source>,
    index: u32,
) -> TestResult<Record<'source>> {
    let (byte, path, kind) = match index {
        0 => (0x11, "rewards/deep", super::fixture::ROOT),
        1 => (0x22, "rewards/pool-a", POOL),
        2 => (0x33, "rewards/pool-b", POOL),
        3 => (0x44, "rewards/other", POOL),
        _ => return Err("unexpected original target position".into()),
    };
    let target = forge.record(index).ok_or("original target Record")?;
    assert_eq!(
        target.guid(),
        Guid::from_stored([byte; 16]),
        "the exact original target GUID token"
    );
    assert_eq!(
        target.record_path()?,
        path,
        "the independently known original target filename"
    );
    assert_eq!(
        target.type_name()?,
        kind,
        "the original Record's actual declared type"
    );
    Ok(target)
}

pub(super) fn leaf<'source>(
    forge: &'source DataForge<'source>,
    node: &StructValue<'source>,
    indices: [Option<u32>; 3],
    chance: f32,
) -> TestResult {
    assert_eq!(node.type_name(), LEAF, "the original reward target type");
    fields(node, &["first", "duplicate", "second", "chance"]);
    let pool = forge
        .structs()
        .find(|structure| structure.name().is_ok_and(|name| name == POOL))
        .ok_or("original pool declaration")?;
    for (name, index) in ["first", "duplicate", "second"].into_iter().zip(indices) {
        declaration(
            node,
            name,
            DataType::Reference,
            false,
            Some(u16::try_from(pool.index())?),
        )?;
        let original = value(node, name)?;
        if let Some(index) = index {
            target(forge, original, index)?;
        } else {
            assert_eq!(
                original,
                &Value::Null,
                "the actual unused Reference slot is null"
            );
        }
    }
    declaration(node, "chance", DataType::Single, false, None)?;
    let Value::Single(actual) = value(node, "chance")? else {
        return Err("the original Single chance is missing".into());
    };
    assert_eq!(
        actual.to_bits(),
        chance.to_bits(),
        "the original Single token bits"
    );
    Ok(())
}

pub(super) fn clean(tree: &p4kx_extraction::RecordTree<'_>, record: &Record<'_>) {
    assert!(
        tree.is_from(record),
        "the tree comes from this exact original Record"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the genuine source has no canonical value errors"
    );
    assert_eq!(
        tree.located_errors().count(),
        0,
        "the complete original tree has no hidden error slots"
    );
}
