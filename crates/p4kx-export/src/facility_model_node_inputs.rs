// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_assembly::RoomModelHelper;
use p4kx_extraction::model::{
    Model, ModelChunkWords, ModelNodeMeshChunk, ModelNodeSelection, ModelNodeSource,
    ModelNodeSources,
};
use p4kx_extraction::{CancelToken, SourceLocation, SourceScope, SourceUse, SourceUseRole};

use super::super::authored_fields::RoomTransformUnavailable;
use super::routes;
use crate::CatalogError;

pub(super) fn select(
    scope: &SourceScope,
    model: &Model,
    nodes: &ModelNodeSources,
    name: &str,
    cancel: Option<&CancelToken>,
    inputs: &mut Vec<SourceUse>,
) -> Result<Result<RoomModelHelper, RoomTransformUnavailable>, CatalogError> {
    let Some(selection) = nodes.selection() else {
        return Ok(Err(RoomTransformUnavailable::Unsupported));
    };
    if !capture_selection(scope, selection, inputs)? {
        return Ok(Err(RoomTransformUnavailable::Unsupported));
    }
    if nodes.uses_synthetic_root() {
        return Ok(Err(RoomTransformUnavailable::Unsupported));
    }
    let mut count = 0;
    for (ordinal, node) in model.nodes.iter().enumerate() {
        routes::check_cancel(cancel)?;
        let Some(source) = nodes.node(ordinal) else {
            return Ok(Err(RoomTransformUnavailable::Unsupported));
        };
        let Some(name_range) = source.name() else {
            return Ok(Err(RoomTransformUnavailable::Unsupported));
        };
        inputs.push(at_node(
            scope,
            ordinal,
            source,
            "name",
            name_range,
            SourceUseRole::Selector,
        )?);
        count += usize::from(node.name.eq_ignore_ascii_case(name));
    }
    let helper = match count {
        0 => return Ok(Err(RoomTransformUnavailable::Absent)),
        1 => RoomModelHelper::find(model, name),
        _ => return Ok(Err(RoomTransformUnavailable::Invalid)),
    };
    let Some(helper) = helper else {
        return Ok(Err(RoomTransformUnavailable::Invalid));
    };
    for (ordinal, node) in nodes.lineage(model, helper.node()) {
        routes::check_cancel(cancel)?;
        record_lineage_node(scope, ordinal, node, inputs)?;
    }
    if helper.transform().finite().is_none() {
        return Ok(Err(RoomTransformUnavailable::Invalid));
    }
    Ok(Ok(helper))
}

fn record_lineage_node(
    scope: &SourceScope,
    ordinal: usize,
    node: &ModelNodeSource,
    inputs: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    if let Some(row) = node.table_row() {
        inputs.push(at_node(
            scope,
            ordinal,
            node,
            "offset",
            row.offset.clone(),
            SourceUseRole::Selector,
        )?);
    }
    if let Some(id) = node.id() {
        inputs.push(at_node(
            scope,
            ordinal,
            node,
            "chunkId",
            id,
            SourceUseRole::Selector,
        )?);
    }
    inputs.push(at_node(
        scope,
        ordinal,
        node,
        "parent",
        node.parent(),
        SourceUseRole::Selector,
    )?);
    inputs.push(at_node(
        scope,
        ordinal,
        node,
        "localTransform",
        node.matrix(),
        SourceUseRole::Conversion,
    )?);
    Ok(())
}

fn capture_selection(
    scope: &SourceScope,
    selection: &ModelNodeSelection,
    inputs: &mut Vec<SourceUse>,
) -> Result<bool, CatalogError> {
    for (field, start, end) in [
        ("signature", 0, 4),
        ("rowCount", 8, 12),
        ("tableOffset", 12, 16),
    ] {
        inputs.push(selector(
            scope,
            "cryEngineChunkHeader",
            field,
            vec![start, end],
        )?);
    }
    match selection {
        ModelNodeSelection::NodeMeshCombo { table, chunk } => {
            if let Some(table) = table {
                table_words(scope, table, "nodeMeshTypePrefix", inputs)?;
            }
            if let Some(chunk) = chunk {
                mesh_chunk(scope, chunk, inputs)?;
            }
        }
        ModelNodeSelection::CrCh { table } => {
            if let Some(table) = table {
                table_words(scope, table, "nodeTypeAndVersionWords", inputs)?;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn table_words(
    scope: &SourceScope,
    table: &ModelChunkWords,
    field: &str,
    inputs: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    let first = table.first_word();
    inputs.push(selector(
        scope,
        "cryEngineChunkTable",
        field,
        vec![first.start, first.end, table.stride(), table.count() as u64],
    )?);
    Ok(())
}

fn mesh_chunk(
    scope: &SourceScope,
    chunk: &ModelNodeMeshChunk,
    inputs: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    let index = chunk.index() as u64;
    let row = chunk.table_row();
    for (field, range) in [("type", &row.kind), ("offset", &row.offset)] {
        inputs.push(selector(
            scope,
            "cryEngineChunkTable",
            field,
            vec![index, range.start, range.end],
        )?);
    }
    for (field, range) in [
        ("nodeCount", chunk.count()),
        ("nameLayout", chunk.name_layout()),
    ] {
        inputs.push(selector(
            scope,
            "cryEngineNodes",
            field,
            vec![index, range.start, range.end],
        )?);
    }
    Ok(())
}

fn at_node(
    scope: &SourceScope,
    ordinal: usize,
    node: &ModelNodeSource,
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, CatalogError> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "modelNodePlacementBytes".to_owned(),
            indices: vec![node.chunk() as u64, ordinal as u64, range.start, range.end],
            field: field.to_owned(),
        },
        role,
    )?)
}

fn selector(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
) -> Result<SourceUse, CatalogError> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices,
            field: field.to_owned(),
        },
        SourceUseRole::Selector,
    )?)
}
