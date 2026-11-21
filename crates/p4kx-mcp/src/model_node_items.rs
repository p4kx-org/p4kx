// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{ModelChunkWords, ModelNodeMeshChunk, ModelNodeSelection, Node};
use p4kx_extraction::{
    FieldOrigins, FieldPath, ModelArchiveSource, NodeInfo, SourceEvidenceError, SourceLocation,
    SourceUse, SourceUseRole,
};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_node_fields::NodeFields;
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
        field_origins(request.primary(), nodes, range, cursor)
    })
}

fn field_origins(
    primary: Option<&ModelArchiveSource>,
    nodes: &[Node],
    range: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let entry = EntryPageSources::from_origin(primary.and_then(ModelArchiveSource::origin))?;
    let sources = primary.and_then(ModelArchiveSource::node_sources);
    let selection = sources.and_then(|sources| sources.selection());
    let (table, chunk, field, known) = match selection {
        Some(ModelNodeSelection::NodeMeshCombo { table, chunk }) => {
            (table.as_ref(), chunk.as_ref(), "nodeMeshTypePrefix", true)
        }
        Some(ModelNodeSelection::CrCh { table }) => {
            (table.as_ref(), None, "nodeTypeAndVersionWords", true)
        }
        _ => (None, None, "nodeTable", false),
    };
    let uses = selection_uses(&entry, known, table, chunk, field)?;
    let mut fields = FieldOrigins::new();
    let root = FieldPath::parse("/structuredContent")?;
    for (field, origin) in [
        ("archive", entry.local()),
        ("model", entry.path.clone()),
        ("section", entry.local()),
    ] {
        model_item_page::put(&mut fields, &root, field, origin)?;
    }
    for field in ["total", "items"] {
        model_item_page::put(&mut fields, &root, field, entry.origin_when(&uses, known))?;
    }
    if cursor.is_some() {
        model_item_page::put(
            &mut fields,
            &root,
            "next_cursor",
            entry.origin_when(&uses, known),
        )?;
    }
    let proof = NodeFields {
        entry: &entry,
        sources,
        chunk,
        selection: &uses,
        complete: known,
    };
    let mut path = root;
    path.push_field("items")?;
    let selected = nodes.get(range.clone()).ok_or(EvidenceError::MissingRows)?;
    for (position, node) in selected.iter().enumerate() {
        path.push_index(position)?;
        proof.write(&mut fields, &path, range.start + position, node)?;
        path.pop();
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: entry.uses.freeze()?,
    })
}

fn selection_uses(
    entry: &EntryPageSources,
    known: bool,
    table: Option<&ModelChunkWords>,
    chunk: Option<&ModelNodeMeshChunk>,
    table_field: &str,
) -> Result<Vec<SourceUse>, SourceEvidenceError> {
    let mut uses = Vec::new();
    if known {
        for (field, start, end) in [
            ("signature", 0, 4),
            ("rowCount", 8, 12),
            ("tableOffset", 12, 16),
        ] {
            uses.push(selector(
                entry,
                "cryEngineChunkHeader",
                field,
                vec![start, end],
            )?);
        }
    }
    if let Some(table) = table {
        let first = table.first_word();
        uses.push(selector(
            entry,
            "cryEngineChunkTable",
            table_field,
            vec![first.start, first.end, table.stride(), table.count() as u64],
        )?);
    }
    if let Some(chunk) = chunk {
        let ordinal = chunk.index() as u64;
        let row = chunk.table_row();
        for (field, bytes) in [("type", &row.kind), ("offset", &row.offset)] {
            uses.push(selector(
                entry,
                "cryEngineChunkTable",
                field,
                vec![ordinal, bytes.start, bytes.end],
            )?);
        }
        let count = chunk.count();
        uses.push(selector(
            entry,
            "cryEngineNodes",
            "nodeCount",
            vec![ordinal, count.start, count.end],
        )?);
    }
    Ok(uses)
}

fn selector(
    entry: &EntryPageSources,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
) -> Result<SourceUse, SourceEvidenceError> {
    entry.subresource(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            field: field.into(),
            indices,
        },
        SourceUseRole::Selector,
    )
}
