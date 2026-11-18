// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{
    Archive, DataForge, InputScope, PhysicalSourceRef, SourceInventory, SourceLocation,
};
use serde_json::Value;

use crate::catalog_read::original::OriginalValuePart;
use crate::record_header_field::RecordHeaderField;

use super::TestResult;
use super::selection::Family;
use super::verify::Context;

pub(super) fn validate(
    context: &Context<'_>,
    inventory: &SourceInventory,
    leaf: &PhysicalSourceRef,
) -> TestResult {
    let owners = &context.census.owners;
    let limit = if context.family == Family::Parents {
        owners.len()
    } else {
        owners.len().min(1)
    };
    validate_record_scope(
        context.archive,
        context.forge,
        inventory,
        leaf,
        &owners[..limit],
    )
}

pub(super) fn validate_record_scope(
    archive: &Archive,
    forge: &DataForge<'_>,
    inventory: &SourceInventory,
    leaf: &PhysicalSourceRef,
    owners: &[u32],
) -> TestResult {
    for usage in inventory.iter_uses() {
        match usage.location() {
            SourceLocation::DataForge { record_index, .. } => {
                body(owners, *record_index)?;
                if usage.source() != leaf || usage.scope() != InputScope::Payload {
                    return Err(
                        "SDK body coordinate is not bound to the original DCB payload".into(),
                    );
                }
            }
            SourceLocation::Subresource {
                namespace, indices, ..
            } => {
                if usage.source() != leaf || usage.scope() != InputScope::Payload {
                    return Err(
                        "SDK DataForge metadata is not bound to its original payload".into(),
                    );
                }
                subresource(forge, owners, namespace, indices)?;
            }
            SourceLocation::Metadata { .. } => {}
            SourceLocation::ArchiveDirectoryPaths {
                start_entry_index,
                end_entry_index,
            }
            | SourceLocation::ArchiveDirectoryTreePaths {
                start_entry_index,
                end_entry_index,
            } => {
                if usage.scope() != InputScope::ArchiveDirectoryMetadata
                    || start_entry_index.get() > end_entry_index.get()
                    || usize::try_from(end_entry_index.get())
                        .ok()
                        .is_none_or(|end| end > archive.entries().len())
                {
                    return Err("SDK directory survey escaped its original metadata extent".into());
                }
            }
            _ => return Err("unexpected source coordinates in the selective SDK witness".into()),
        }
    }
    Ok(())
}

fn body(owners: &[u32], index: u32) -> TestResult {
    if owners.binary_search(&index).is_err() {
        return Err("SDK body evidence escaped its explicit owner/group-family scope".into());
    }
    Ok(())
}

fn subresource(
    forge: &DataForge<'_>,
    owners: &[u32],
    namespace: &str,
    indices: &[u64],
) -> TestResult {
    let first = indices
        .first()
        .copied()
        .ok_or("DataForge source coordinate requires an index")?;
    let index = u32::try_from(first).map_err(|_| "DataForge source index is out of range")?;
    match namespace {
        OriginalValuePart::NAMESPACE
        | "dataforgeRecordDeclarations"
        | "dataforgeTraversal"
        | "dataforgeRecordBody" => body(owners, index),
        RecordHeaderField::NAMESPACE if indices.len() == 1 => {
            forge
                .record(index)
                .ok_or("original Reference-target header missing")?;
            Ok(())
        }
        "dataforgeStruct" | "dataforgeStructDeclaration" => {
            forge
                .struct_at(index)
                .ok_or("original Struct declaration missing")?;
            Ok(())
        }
        "dataforgeEnum" => {
            let ordinal =
                usize::try_from(first).map_err(|_| "Enum declaration index out of range")?;
            forge
                .enums()
                .nth(ordinal)
                .ok_or("original Enum declaration missing")?;
            Ok(())
        }
        _ => Err("unexpected DataForge body or declaration namespace in SDK witness".into()),
    }
}

pub(super) fn row(context: &Context<'_>, row: &Value) -> TestResult {
    check_targets(context.forge, row)?;
    let Some(arena) = row.get("fields") else {
        return Ok(());
    };
    let roots = arena
        .get("roots")
        .ok_or("original owner arena roots missing")?;
    let values = arena
        .get("values")
        .and_then(Value::as_array)
        .ok_or("original owner arena values missing")?;
    let mut pending = Vec::new();
    links(roots, &mut pending)?;
    let mut reached = BTreeSet::new();
    while let Some(index) = pending.pop() {
        let index =
            usize::try_from(index).map_err(|_| "owner-local value reference out of range")?;
        if !reached.insert(index) {
            return Err("owner-local value graph repeats an occurrence or contains a cycle".into());
        }
        let node = values
            .get(index)
            .ok_or("owner-local value reference is unresolved")?;
        links(node, &mut pending)?;
    }
    if reached.len() != values.len() {
        return Err("owner-local value inventory contains unreachable occurrences".into());
    }
    Ok(())
}

fn links(value: &Value, result: &mut Vec<u64>) -> TestResult {
    match value {
        Value::Array(values) => {
            for value in values {
                links(value, result)?;
            }
        }
        Value::Object(fields) => {
            if let Some(index) = fields.get("valueIndex") {
                let index = index
                    .as_str()
                    .and_then(|text| text.parse::<u64>().ok())
                    .ok_or("owner-local reference must retain its exact decimal index")?;
                result.push(index);
            } else {
                for value in fields.values() {
                    links(value, result)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn check_targets(forge: &DataForge<'_>, value: &Value) -> TestResult {
    match value {
        Value::Array(values) => {
            for value in values {
                check_targets(forge, value)?;
            }
        }
        Value::Object(fields) => {
            if let Some(index) = value
                .pointer("/targetRecordIndex/value")
                .and_then(Value::as_u64)
            {
                let index =
                    u32::try_from(index).map_err(|_| "target physical index out of range")?;
                let record = forge
                    .record(index)
                    .ok_or("original target metadata missing")?;
                if let Some(guid) = value.pointer("/guid/value").and_then(Value::as_str)
                    && guid != record.guid().to_string()
                {
                    return Err(
                        "SDK Reference target differs from original physical header GUID".into(),
                    );
                }
                if let Some(source_type) =
                    value.pointer("/targetType/value").and_then(Value::as_str)
                    && record.type_name().ok() != Some(source_type)
                {
                    return Err("SDK Reference target differs from original declared type".into());
                }
            }
            for value in fields.values() {
                check_targets(forge, value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
