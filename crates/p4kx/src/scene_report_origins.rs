// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_assembly::EntityNode;
use p4kx_extraction::{
    Archive, DataForge, DataForgeOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope, Record,
    RecordJsonLocation, RecordText, SourceInput, SourceLocation, SourceScope, SourceUseRole,
    SourceUses,
};
use serde_json::Value;

pub(crate) fn entity_identity(
    fields: &mut FieldOrigins,
    uses: &SourceUses,
    forge: &DataForge<'_>,
    record: &Record<'_>,
    original: &DataForgeOrigin,
    tree: &EntityNode,
) -> Result<()> {
    if !record.is_from(forge)
        || Some(record.index()) != tree.record_index()
        || record.guid() != tree.guid()
    {
        for path in ["/entity", "/guid"] {
            fields.insert(FieldPath::parse(path)?, uses.origin(&[], false))?;
        }
        return Ok(());
    }
    let (scope, complete) = dataforge_scope(uses, original)?;
    let guid = scope.at(
        SourceLocation::DataForge {
            record_index: record.index(),
            field: "guid".into(),
        },
        SourceUseRole::Payload,
    )?;
    fields.insert(FieldPath::parse("/guid")?, uses.origin(&[guid], complete))?;
    fields.insert(
        FieldPath::parse("/entity")?,
        record_name(uses, &scope, forge, record, complete)?,
    )?;
    Ok(())
}

pub(crate) fn dataforge_scope(
    uses: &SourceUses,
    original: &DataForgeOrigin,
) -> Result<(SourceScope, bool)> {
    let (input, complete) = match original {
        DataForgeOrigin::Archive(entry) => (
            SourceInput::Archive(entry),
            entry.ancestors().all(|ancestor| ancestor.is_some()),
        ),
        DataForgeOrigin::File(file) => (SourceInput::File(file), true),
        _ => (SourceInput::Unknown, false),
    };
    Ok((uses.observe(input, InputScope::Payload)?, complete))
}

pub(crate) fn record_name(
    uses: &SourceUses,
    scope: &SourceScope,
    forge: &DataForge<'_>,
    record: &Record<'_>,
    source_complete: bool,
) -> Result<FieldOrigin> {
    let mut name = vec![scope.at(
        SourceLocation::DataForge {
            record_index: record.index(),
            field: "recordName".into(),
        },
        SourceUseRole::Conversion,
    )?];
    let name_complete = match record.stored_name() {
        Ok(RecordText::Present(text)) => {
            let original = RecordJsonLocation::borrowed_text(forge, text)
                .map(|location| scope.at(location, SourceUseRole::Conversion))
                .transpose()?;
            let complete = original.is_some();
            name.extend(original);
            complete
        }
        Ok(RecordText::Absent | RecordText::Null) => true,
        _ => false,
    };
    Ok(uses.origin(&name, source_complete && name_complete))
}

/// Binds the report paths to the original directory entries opened in order.
pub(crate) fn container_paths(
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    layers: &[Archive],
    chain: &[String],
    mut on_opened: impl FnMut(&SourceScope) -> Result<()>,
) -> Result<()> {
    ensure!(
        layers.len() == chain.len() + 1,
        "opened container path count differs"
    );
    let mut path_uses = Vec::with_capacity(chain.len());
    let mut complete = true;
    for (index, (layer, path)) in layers.iter().skip(1).zip(chain).enumerate() {
        let at = FieldPath::parse(&format!("/containers/{index}"))?;
        let Some(entry) = layer.container_input_origin() else {
            complete = false;
            fields.insert(at, uses.origin(&[], false))?;
            continue;
        };
        ensure!(
            entry.entry().path() == path,
            "opened container path differs from its original Entry"
        );
        let payload = uses.observe(SourceInput::Archive(&entry), InputScope::Payload)?;
        let path = payload.for_scope(InputScope::ArchiveDirectoryMetadata).at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        on_opened(&payload)?;
        let entry_complete = entry.ancestors().all(|ancestor| ancestor.is_some());
        complete &= entry_complete;
        fields.insert(at, uses.origin(std::slice::from_ref(&path), entry_complete))?;
        path_uses.push(path);
    }
    fields.insert(
        FieldPath::parse("/containers")?,
        uses.origin(&path_uses, complete),
    )?;
    Ok(())
}

pub(crate) fn cover_leaves(
    fields: &mut FieldOrigins,
    value: &Value,
    path: &mut FieldPath,
    inherited: &FieldOrigin,
) -> Result<()> {
    let origin = fields.get(path).unwrap_or(inherited).clone();
    match value {
        Value::Object(members) if !members.is_empty() => {
            for (name, member) in members {
                path.push_field(name)?;
                cover_leaves(fields, member, path, &origin)?;
                path.pop();
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (index, item) in items.iter().enumerate() {
                path.push_index(index)?;
                cover_leaves(fields, item, path, &origin)?;
                path.pop();
            }
        }
        _ if fields.get(path).is_none() => fields.insert(path.clone(), origin)?,
        _ => {}
    }
    Ok(())
}
