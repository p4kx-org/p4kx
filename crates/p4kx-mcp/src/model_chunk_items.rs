// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model_info;
use p4kx_extraction::{ChunkInfo, FieldOrigins, FieldPath, SourceUses};

use crate::error::ToolError;
use crate::model_item_page::{EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(super) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let chunks = &request.source.model().chunks;
    let values = request
        .page
        .slice(chunks)
        .iter()
        .map(|chunk| serde_json::to_value(ChunkInfo::of(chunk)).map_err(ToolError::from));
    request.publish(chunks.len(), values, |range, _| {
        field_origins(request, range)
    })
}

fn field_origins(
    request: &ModelItemPage<'_>,
    selected: Range<usize>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    let mut path = FieldPath::parse("/structuredContent/items")?;
    for (local, index) in selected.enumerate() {
        path.push_index(local)?;
        model_info::write_chunk_type_origin(request.source, index, &path, &uses, &mut fields)
            .map_err(EvidenceError::Chunk)?;
        path.pop();
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}
