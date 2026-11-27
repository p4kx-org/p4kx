// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model_info;
use p4kx_extraction::{ChunkInfo, FieldOrigins, FieldPath};

use crate::content_origin::EntryPageSources;

use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(super) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let chunks = &request.source.model().chunks;
    let values = request
        .page
        .slice(chunks)
        .iter()
        .map(|chunk| serde_json::to_value(ChunkInfo::of(chunk)).map_err(ToolError::from));
    request.publish(chunks.len(), values, |range, cursor| {
        field_origins(request, range, cursor.is_some())
    })
}

fn field_origins(
    request: &ModelItemPage<'_>,
    selected: Range<usize>,
    next: bool,
) -> Result<ModelItemOrigins, EvidenceError> {
    let model =
        EntryPageSources::from_origin(request.primary().and_then(|source| source.origin()))?;
    let navigation = model_info::model_chunk_count_origin(request.source, &model.uses)
        .map_err(EvidenceError::Chunk)?;
    let root = FieldPath::parse("/structuredContent")?;
    let mut fields = FieldOrigins::new();
    let mut path = FieldPath::parse("/structuredContent/items")?;
    for (local, index) in selected.enumerate() {
        path.push_index(local)?;
        model_info::write_chunk_origins(request.source, index, &path, &model.uses, &mut fields)
            .map_err(EvidenceError::Chunk)?;
        path.pop();
    }
    let items = if fields.iter().next().is_none() {
        navigation.clone()
    } else {
        model
            .uses
            .origin_with_origins(fields.iter().map(|(_, origin)| origin), false)?
    };
    for (name, origin) in [
        ("archive", model.local()),
        ("model", model.path.clone()),
        ("section", model.local()),
        ("items", items),
        ("total", navigation.clone()),
    ] {
        model_item_page::put(&mut fields, &root, name, origin)?;
    }
    if next {
        model_item_page::put(&mut fields, &root, "next_cursor", navigation)?;
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: model.uses.freeze()?,
    })
}
