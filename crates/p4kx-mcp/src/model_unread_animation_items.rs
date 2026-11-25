// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    ArchiveModelAnimations, FieldOrigins, FieldPath, PathIndex, PositionedUnreadAnimation,
    SourceUse, SourceUseRole, UnreadInfo,
};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};
use crate::models;
use p4kx_extraction::model_info::animation_origin_sites::{self, AnimationChunkSites};

pub(crate) fn list(request: &ModelItemPage<'_>, path: &str) -> Result<ModelItemsResult, ToolError> {
    let index = request.opened.index();
    let unread = ArchiveModelAnimations::read_unread(&index, request.source.name())
        .map_err(|error| models::converted(path, "a model with its animations", error))?;
    let values = request
        .page
        .slice(&unread)
        .iter()
        .map(|row| serde_json::to_value(UnreadInfo::of(row.value())).map_err(ToolError::from));
    request.publish(unread.len(), values, |range, cursor| {
        origins(request, &index, &unread, range, cursor)
    })
}

fn origins(
    request: &ModelItemPage<'_>,
    index: &PathIndex<'_>,
    unread: &[PositionedUnreadAnimation],
    range: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let model =
        EntryPageSources::from_origin(request.primary().and_then(|source| source.origin()))?;
    let page = unread.get(range).ok_or(EvidenceError::MissingRows)?;
    let model_use = model.leaf_path()?;
    let navigation = model.origin_when(std::slice::from_ref(&model_use), false);
    let mut aggregate = vec![model_use];
    let mut fields = FieldOrigins::new();
    let root = FieldPath::parse("/structuredContent")?;
    for (name, origin) in [
        ("archive", model.local()),
        ("model", model.path.clone()),
        ("section", model.local()),
        ("total", navigation.clone()),
    ] {
        model_item_page::put(&mut fields, &root, name, origin)?;
    }
    if cursor.is_some() {
        model_item_page::put(&mut fields, &root, "next_cursor", navigation)?;
    }
    let mut path = root.clone();
    path.push_field("items")?;
    for (position, row) in page.iter().enumerate() {
        path.push_index(position)?;
        // The decoder and page resolve the label in the same immutable path index.
        let matched = index
            .find(&row.value().source)
            .filter(|entry| entry.path() == row.value().source);
        if let Some(entry) = matched {
            let entry = EntryPageSources::observe_into(index.archive(), entry, model.uses.clone())?;
            write_row(&mut fields, &path, &entry, row, &mut aggregate)?;
        } else {
            for field in ["source", "name", "reason"] {
                model_item_page::put(&mut fields, &path, field, model.origin_when(&[], false))?;
            }
        }
        path.pop();
    }
    model_item_page::put(
        &mut fields,
        &root,
        "items",
        model.origin_when(&aggregate, false),
    )?;
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: model.uses.freeze()?,
    })
}

fn write_row(
    fields: &mut FieldOrigins,
    path: &FieldPath,
    entry: &EntryPageSources,
    row: &PositionedUnreadAnimation,
    aggregate: &mut Vec<SourceUse>,
) -> Result<(), EvidenceError> {
    model_item_page::put(fields, path, "source", entry.path.clone())?;
    aggregate.extend_from_slice(&entry.path_uses);
    let unknown = entry.origin_when(&[], false);
    let Some(coordinates) = row.coordinates() else {
        model_item_page::put(fields, path, "name", unknown.clone())?;
        return model_item_page::put(fields, path, "reason", unknown);
    };
    let metadata = coordinates.metadata_chunk();
    let selectors = AnimationChunkSites::of(entry.payload_scope(), metadata)?;
    let site = |field, bytes, role| {
        animation_origin_sites::clip_site(
            entry.payload_scope(),
            coordinates.ordinal(),
            metadata.ordinal(),
            field,
            bytes,
            role,
        )
    };
    let name = match coordinates.database_path() {
        Some(bytes) => {
            let mut uses = selectors.selectors.clone();
            uses.push(site("databasePath", bytes, SourceUseRole::Conversion)?);
            aggregate.extend_from_slice(&uses);
            entry.origin_when(&uses, false)
        }
        None => entry.path.clone(),
    };
    let reason = match coordinates.additive_flags() {
        Some(bytes) => {
            let mut uses = selectors.selectors;
            uses.push(site("additive", bytes, SourceUseRole::Selector)?);
            aggregate.extend_from_slice(&uses);
            entry.origin_when(&uses, false)
        }
        None => unknown,
    };
    model_item_page::put(fields, path, "name", name)?;
    model_item_page::put(fields, path, "reason", reason)
}
