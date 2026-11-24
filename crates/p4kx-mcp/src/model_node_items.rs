// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model_info::ModelNodeOrigins;
use p4kx_extraction::{FieldOrigins, FieldPath, ModelArchiveSource, NodeInfo};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(crate) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let nodes = &request.source.model().nodes;
    let values = request
        .page
        .slice(nodes)
        .iter()
        .enumerate()
        .map(|(position, node)| {
            serde_json::to_value(NodeInfo::of_node(request.page.offset() + position, node))
                .map_err(ToolError::from)
        });
    request.publish(nodes.len(), values, |range, cursor| {
        field_origins(request, range, cursor)
    })
}

fn field_origins(
    request: &ModelItemPage<'_>,
    range: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let entry =
        EntryPageSources::from_origin(request.primary().and_then(ModelArchiveSource::origin))?;
    let proof =
        ModelNodeOrigins::capture(request.source, &entry.uses).map_err(EvidenceError::Node)?;
    let mut fields = FieldOrigins::new();
    let root = FieldPath::parse("/structuredContent")?;
    for (field, origin) in [
        ("archive", entry.local()),
        ("model", entry.path.clone()),
        ("section", entry.local()),
    ] {
        model_item_page::put(&mut fields, &root, field, origin)?;
    }
    let selection = proof.collection_origin();
    for field in ["total", "items"] {
        model_item_page::put(&mut fields, &root, field, selection.clone())?;
    }
    if cursor.is_some() {
        model_item_page::put(&mut fields, &root, "next_cursor", selection)?;
    }
    let mut path = root;
    path.push_field("items")?;
    for (position, ordinal) in range.enumerate() {
        path.push_index(position)?;
        proof
            .write_node(&mut fields, &path, ordinal)
            .map_err(EvidenceError::Node)?;
        path.pop();
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: entry.uses.freeze()?,
    })
}
