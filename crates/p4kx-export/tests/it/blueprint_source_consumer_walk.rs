// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    SourceArenaOutput, SourceField, SourceNode, SourcePayload, SourceRecordReference,
    SourceValueRef, SourceWeakReference, SourceWeakTarget,
};
use p4kx_extraction::Location;

use super::TestResult;
use crate::canonical_header::wire::Wire;

#[path = "blueprint_error_consumer_walk.rs"]
mod errors;

pub(crate) fn visit_arena(arena: &SourceArenaOutput<'_>, wire: Wire<'_>) -> TestResult {
    let roots = wire.field("roots")?;
    roots.len(arena.roots().len())?;
    for (index, field) in arena.roots().iter().enumerate() {
        visit_field(field, arena, roots.index(index)?)?;
    }
    let values = wire.field("values")?;
    values.len(arena.values().len())?;
    for (index, node) in arena.values().iter().enumerate() {
        visit_node(node, arena, values.index(index)?)?;
    }
    Ok(())
}

pub(crate) fn visit_ref(
    reference: SourceValueRef,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    wire.field("valueIndex")?
        .raw(&reference.value_index().to_string())?;
    assert!(
        arena.value(reference).is_some(),
        "original navigation resolves only within its owner arena"
    );
    Ok(())
}

fn visit_field(
    field: &SourceField<'_>,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    wire.field("name")?.leaf(field.name())?;
    let declaration = wire.field("declaration")?;
    declaration
        .field("dataType")?
        .leaf(field.declaration().data_type())?;
    declaration
        .field("isArray")?
        .leaf(field.declaration().is_array())?;
    declaration
        .field("target")?
        .leaf(field.declaration().target())?;
    visit_ref(field.value(), arena, wire.field("value")?)
}

fn visit_node(node: &SourceNode<'_>, arena: &SourceArenaOutput<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("value")?.cell(node.value(), |payload, wire| {
        visit_payload(payload, arena, wire)
    })?;
    let original_error = wire.field("originalError")?;
    node.original_error().map_or_else(
        || original_error.raw(&node.original_error()),
        |error| errors::visit_error(error, original_error),
    )
}

fn visit_payload(
    payload: &SourcePayload<'_>,
    arena: &SourceArenaOutput<'_>,
    wire: Wire<'_>,
) -> TestResult {
    wire.raw(payload)?;
    match payload {
        SourcePayload::Struct {
            source_type,
            fields,
        } => {
            wire.field("sourceType")?.leaf(source_type)?;
            let expected = wire.field("fields")?;
            expected.len(fields.len())?;
            for (index, field) in fields.iter().enumerate() {
                visit_field(field, arena, expected.index(index)?)?;
            }
            Ok(())
        }
        SourcePayload::Array { elements } => {
            let expected = wire.field("elements")?;
            expected.len(elements.len())?;
            for (index, reference) in elements.iter().enumerate() {
                visit_ref(*reference, arena, expected.index(index)?)?;
            }
            Ok(())
        }
        SourcePayload::Boolean { value } => wire.field("value")?.raw(value),
        SourcePayload::Number { value } => wire.field("value")?.number(value),
        SourcePayload::String { value } | SourcePayload::Locale { value } => {
            let text = wire.field("value")?;
            text.raw(value)?;
            text.field("sourceText")?.raw(value.as_str())
        }
        SourcePayload::Enum { value } => wire.field("value")?.raw(value),
        SourcePayload::Guid { value } => wire.field("value")?.raw(&value.get().to_string()),
        SourcePayload::RecordReference { value } => visit_record(value, wire.field("value")?),
        SourcePayload::WeakReference { value } => visit_weak(value, wire.field("value")?),
        _ => Err("new original payloads require a typed traversal".into()),
    }
}

fn visit_record(reference: &SourceRecordReference<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("guid")?.guid(&reference.guid())?;
    wire.field("targetRecordIndex")?
        .leaf(reference.target_record_index())?;
    wire.field("recordPath")?.leaf(reference.record_path())?;
    wire.field("targetType")?.leaf(reference.target_type())?;
    wire.field("status")?.raw(reference.status())
}

fn visit_weak(reference: &SourceWeakReference<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("targetRecordIndex")?
        .leaf(reference.target_record_index())?;
    wire.field("recordGuid")?
        .cell(reference.record_guid(), |guid, wire| {
            wire.raw(&guid.get().to_string())
        })?;
    wire.field("recordPath")?.leaf(reference.record_path())?;
    let target = wire.field("target")?;
    match reference.target() {
        SourceWeakTarget::Record { source_type } => {
            target.tag("record")?;
            assert!(
                reference.instance_steps().is_none(),
                "a Record pointer has no owned-instance steps"
            );
            target.field("sourceType")?.leaf(source_type)
        }
        SourceWeakTarget::Instance {
            source_type,
            location,
        } => {
            target.tag("instance")?;
            target.field("sourceType")?.leaf(source_type)?;
            target.field("location")?.cell(location, |location, wire| {
                assert_eq!(
                    Some(location.steps()),
                    reference.instance_steps(),
                    "owned-instance steps retain their actual source location"
                );
                wire.raw(Location::new(location.steps()).as_str())
            })
        }
        _ => Err("new weak targets require a typed traversal".into()),
    }
}
