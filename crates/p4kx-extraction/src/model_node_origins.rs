// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_model::{
    ModelChunkWords, ModelFile, ModelNodeMeshChunk, ModelNodeSelection, ModelNodeSources, Node,
};

use crate::model_geometry_scope::ModelGeometryScope;
use crate::source_provenance::ChunkHeaderField;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, ModelArchiveSource, ModelSource, SourceLocation,
    SourceUse, SourceUseRole, SourceUses,
};

#[path = "model_node_origin_fields.rs"]
mod fields;

/// Writes the original node selection and row evidence into a caller's inventory.
pub struct ModelNodeOrigins<'a> {
    entry: NodeEntryOrigins<'a>,
    nodes: &'a [Node],
    sources: Option<&'a ModelNodeSources>,
    chunk: Option<&'a ModelNodeMeshChunk>,
    selection: Vec<SourceUse>,
    complete: bool,
}

impl<'a> ModelNodeOrigins<'a> {
    /// Borrows original node coordinates already captured by the model read.
    ///
    /// # Errors
    /// Returns a source evidence error.
    pub fn capture(source: &'a ModelSource, uses: &'a SourceUses) -> io::Result<Self> {
        let scope = ModelGeometryScope::capture(uses, source, ModelFile::Model)?;
        let entry = NodeEntryOrigins { uses, scope };
        let sources = entry.scope.as_ref().and_then(|_| {
            source
                .archive_sources()
                .iter()
                .find(|source| source.file() == ModelFile::Model)
                .and_then(ModelArchiveSource::node_sources)
        });
        let (table, chunk, field, complete) = match sources.and_then(ModelNodeSources::selection) {
            Some(ModelNodeSelection::NodeMeshCombo { table, chunk }) => {
                (table.as_ref(), chunk.as_ref(), "nodeMeshTypePrefix", true)
            }
            Some(ModelNodeSelection::CrCh { table }) => {
                (table.as_ref(), None, "nodeTypeAndVersionWords", true)
            }
            _ => (None, None, "nodeTable", false),
        };
        let selection = if complete {
            selection_uses(&entry, table, chunk, field)?
        } else {
            Vec::new()
        };
        Ok(Self {
            entry,
            nodes: &source.model().nodes,
            sources,
            chunk,
            selection,
            complete,
        })
    }

    /// Returns the original selection evidence for the node collection or count.
    #[must_use]
    pub fn collection_origin(&self) -> FieldOrigin {
        self.entry.origin_when(&self.selection, self.complete)
    }
}

struct NodeEntryOrigins<'a> {
    uses: &'a SourceUses,
    scope: Option<ModelGeometryScope>,
}

impl NodeEntryOrigins<'_> {
    fn scope(&self) -> io::Result<&ModelGeometryScope> {
        self.scope
            .as_ref()
            .ok_or_else(|| io::Error::other("model node evidence has no original Entry"))
    }

    fn origin_when(&self, actual: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(
            actual,
            complete && self.scope.as_ref().is_some_and(|scope| scope.complete),
        )
    }

    fn subresource(&self, location: SourceLocation, role: SourceUseRole) -> io::Result<SourceUse> {
        self.scope()?
            .scope()
            .at(location, role)
            .map_err(io::Error::other)
    }

    fn leaf_path(&self) -> io::Result<SourceUse> {
        self.scope()?.directory_path(SourceUseRole::Conversion)
    }
}

fn selection_uses(
    entry: &NodeEntryOrigins<'_>,
    table: Option<&ModelChunkWords>,
    chunk: Option<&ModelNodeMeshChunk>,
    table_field: &str,
) -> io::Result<Vec<SourceUse>> {
    let mut uses = Vec::new();
    for field in [
        ChunkHeaderField::Signature,
        ChunkHeaderField::RowCount,
        ChunkHeaderField::TableOffset,
    ] {
        uses.push(
            field
                .observe(entry.scope()?.scope(), SourceUseRole::Selector)
                .map_err(io::Error::other)?,
        );
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
    entry: &NodeEntryOrigins<'_>,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
) -> io::Result<SourceUse> {
    entry.subresource(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            field: field.into(),
            indices,
        },
        SourceUseRole::Selector,
    )
}

fn put(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> io::Result<()> {
    let mut path = parent.clone();
    path.push_field(name).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
