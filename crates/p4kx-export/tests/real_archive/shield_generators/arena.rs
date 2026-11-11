// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataType, Field, StructValue, Value as Original};
use serde_json::Value;

use crate::original_item_scalar as scalar;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

pub(super) fn source_type<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    source: &StructValue<'source>,
    exported: &Value,
) -> TestResult {
    let kind = cell(exported, "sourceType")?;
    require_state(kind, "present")?;
    assert!(
        proof::text(kind, "/value")? == source.type_name(),
        "the concrete type follows its original Class"
    );
    trace.part(kind, raw, "$type", "selector")
}

pub(super) struct Coverage {
    pub(super) roots: usize,
    pub(super) nodes: usize,
}

pub(super) fn additional<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    exported: &Value,
    mut consumed: BTreeSet<&'static str>,
) -> TestResult<Coverage> {
    let arena = cell(exported, "additionalFields")?;
    let fields = source
        .fields()
        .iter()
        .filter(|field| !consumed.remove(field.name()));
    compare(trace, fields, arena)
}

pub(super) fn compare<'read, 'source: 'read>(
    trace: &Trace<'_, 'source>,
    fields: impl Iterator<Item = &'read Field<'source>>,
    arena: &Value,
) -> TestResult<Coverage> {
    let mut pending = Vec::new();
    let nodes = proof::rows(arena, "/values")?;
    let roots = proof::rows(arena, "/roots")?;
    push_fields(trace, fields, roots, &mut pending)?;
    let mut visited = BTreeSet::new();
    while let Some((raw, reference)) = pending.pop() {
        let index = proof::text(reference, "/valueIndex")?
            .parse::<usize>()
            .map_err(|_| "local authored value index")?;
        assert!(
            visited.insert(index),
            "a local authored occurrence has one parent"
        );
        let node = nodes.get(index).ok_or("local authored value")?;
        compare_node(trace, raw, node, &mut pending)?;
    }
    assert!(
        visited.len() == nodes.len(),
        "every authored node is reachable"
    );
    Ok(Coverage {
        roots: roots.len(),
        nodes: nodes.len(),
    })
}

type Pending<'read, 'source, 'catalog> = Vec<(&'read Original<'source>, &'catalog Value)>;

fn push_fields<'read, 'source: 'read, 'catalog>(
    trace: &Trace<'_, 'source>,
    original: impl Iterator<Item = &'read Field<'source>>,
    exported: &'catalog [Value],
    pending: &mut Pending<'read, 'source, 'catalog>,
) -> TestResult {
    let mut projected = exported.iter();
    for field in original {
        let output = projected
            .next()
            .ok_or("one authored root for each physical field")?;
        assert!(
            proof::text(output, "/name/value")? == field.name(),
            "authored field order and spelling match the source"
        );
        trace.part(
            cell(output, "name")?,
            field.value(),
            "$fieldName",
            "payload",
        )?;
        declaration(trace, field, cell(output, "declaration")?)?;
        pending.push((field.value(), cell(output, "value")?));
    }
    assert!(projected.next().is_none(), "no extra authored fields");
    Ok(())
}

fn declaration<'source>(
    trace: &Trace<'_, 'source>,
    original: &Field<'source>,
    exported: &Value,
) -> TestResult {
    let storage = original.storage();
    assert!(
        proof::text(exported, "/dataType/value")? == data_type(storage.data_type())
            && exported.pointer("/isArray/value").and_then(Value::as_bool)
                == Some(storage.is_array())
            && proof::number(exported, "/target/value")? == u64::from(storage.target()),
        "authored field storage matches the original declaration"
    );
    for (key, part) in [
        ("dataType", "$dataType"),
        ("isArray", "$isArray"),
        ("target", "$target"),
    ] {
        trace.part(cell(exported, key)?, original.value(), part, "selector")?;
    }
    Ok(())
}

const fn data_type(value: DataType) -> &'static str {
    match value {
        DataType::Bool => "bool",
        DataType::Int8 => "int8",
        DataType::Int16 => "int16",
        DataType::Int32 => "int32",
        DataType::Int64 => "int64",
        DataType::UInt8 => "uint8",
        DataType::UInt16 => "uint16",
        DataType::UInt32 => "uint32",
        DataType::UInt64 => "uint64",
        DataType::Single => "single",
        DataType::Double => "double",
        DataType::String => "string",
        DataType::Locale => "locale",
        DataType::Guid => "guid",
        DataType::Enum => "enum",
        DataType::Class => "class",
        DataType::StrongPointer => "strongPointer",
        DataType::WeakPointer => "weakPointer",
        DataType::Reference => "reference",
    }
}

fn compare_node<'read, 'source, 'catalog>(
    trace: &Trace<'_, 'source>,
    raw: &'read Original<'source>,
    node: &'catalog Value,
    pending: &mut Pending<'read, 'source, 'catalog>,
) -> TestResult {
    let value = cell(node, "value")?;
    let error = cell(node, "originalError")?;
    if matches!(raw, Original::Error(_)) {
        require_state(value, "unreadable")?;
        assert!(
            error.is_object() && cell(value, "value")?.is_null(),
            "unreadable original nodes retain their error facts"
        );
        return trace.attempted_value(value, raw);
    }
    assert!(
        error.is_null(),
        "readable original nodes have no decode error"
    );
    trace.check(value, raw, "payload")?;
    let body = cell(value, "value")?;
    if matches!(raw, Original::Null) {
        require_state(value, "null")?;
        assert!(body.is_null(), "original null remains null");
        return Ok(());
    }
    require_state(value, "present")?;
    match raw {
        Original::Struct(structure) => {
            assert!(
                proof::text(body, "/type")? == "struct",
                "original struct node"
            );
            source_type(trace, raw, structure, body)?;
            push_fields(
                trace,
                structure.fields().iter(),
                proof::rows(body, "/fields")?,
                pending,
            )
        }
        Original::Array(elements) => {
            assert!(
                proof::text(body, "/type")? == "array",
                "original array node"
            );
            let projected = proof::rows(body, "/elements")?;
            assert!(
                elements.len() == projected.len(),
                "all original array elements are retained"
            );
            pending.extend(elements.iter().zip(projected));
            Ok(())
        }
        _ => scalar::compare(raw, body),
    }
}
