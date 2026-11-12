// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_chunks::{ByteOrder, ChunkFile, crch};
use p4kx_foundation::ByteReader;

use crate::build::corrupt;
use crate::error::failure;
use crate::hierarchy::find_cycle;
use crate::reader::ReadValues;
use crate::{
    CorruptChunk, ModelError, ModelFile, ModelNodeSource, ModelNodeSources, Node, Transform,
    UnsupportedLayout,
};

const NODE_VERSION: u32 = 0x824;
const NAME_LEN: usize = 64;
const NO_PARENT: u32 = u32::MAX;
const UNUSED_WORDS: usize = 3 * 4;
const PARENT_AT: u64 = NAME_LEN as u64 + 4;
const MATRIX_AT: u64 = PARENT_AT + 4 + UNUSED_WORDS as u64;
const MATRIX_LEN: u64 = 16 * 4;

/// A `CrCh` node with the chunk ID its mesh or helper is referred to by.
#[derive(Debug, Clone)]
pub(crate) struct CrchNode {
    pub(crate) object: u32,
    pub(crate) node: Node,
}

struct Raw {
    object: u32,
    parent: u32,
    name: String,
    local: Transform,
}

/// Reads the version `0x824` `Node` chunks of `file`, in table order: name, object chunk ID,
/// parent chunk ID and the matrix, which is the transform relative to the parent.
pub(crate) fn read_nodes<B: AsRef<[u8]>>(
    file_name: &str,
    file: &ChunkFile<B>,
    read: &mut Vec<(ModelFile, usize)>,
    mut sources: Option<&mut ModelNodeSources>,
) -> Result<Vec<CrchNode>, ModelError> {
    let mut raw = Vec::new();
    let mut by_id = HashMap::new();
    for (index, chunk) in file.chunks_of(crch::NODE) {
        if chunk.version() != NODE_VERSION {
            continue;
        }
        let fail = corrupt(file_name, index, "Node");
        if chunk.byte_order() != ByteOrder::Little {
            return Err(failure(file_name, index, "Node")(
                UnsupportedLayout::BigEndian.into(),
            ));
        }
        let chunk_id = chunk.id().unwrap_or_default();
        let observe = |name_len: usize| {
            if let Some(sources) = sources.as_mut() {
                sources.push(node_source(file, index, chunk.offset(), name_len));
            }
        };
        let record =
            read_record(file.chunk_data(index).unwrap_or_default(), observe).map_err(&fail)?;
        if by_id.insert(chunk_id, raw.len()).is_some() {
            return Err(fail(CorruptChunk::RepeatedChunkId(chunk_id)));
        }
        read.push((ModelFile::Model, index));
        raw.push((index, record));
    }

    let mut nodes = Vec::with_capacity(raw.len());
    for (position, (index, record)) in raw.iter().enumerate() {
        let parent = if record.parent == NO_PARENT {
            None
        } else {
            let found = by_id.get(&record.parent).copied();
            Some(found.ok_or_else(|| {
                corrupt(file_name, *index, "Node")(CorruptChunk::ParentId {
                    node: position,
                    parent: record.parent,
                })
            })?)
        };
        nodes.push(Node::new(record.name.clone(), parent, record.local));
    }
    let first = raw.first().map_or(0, |&(index, _)| index);
    let parents: Vec<Option<usize>> = nodes.iter().map(|node| node.parent).collect();
    if let Some(node) = find_cycle(&parents) {
        return Err(corrupt(file_name, first, "Node")(
            CorruptChunk::ParentCycle(node),
        ));
    }

    Ok(nodes
        .into_iter()
        .zip(&raw)
        .map(|(node, (_, record))| CrchNode {
            object: record.object,
            node,
        })
        .collect())
}

fn node_source<B: AsRef<[u8]>>(
    file: &ChunkFile<B>,
    index: usize,
    start: u64,
    name_len: usize,
) -> ModelNodeSource {
    let name_extent = (name_len + 1).min(NAME_LEN);
    ModelNodeSource {
        chunk: index,
        name: Some(start..start + name_extent as u64),
        parent: start + PARENT_AT..start + PARENT_AT + 4,
        matrix: start + MATRIX_AT..start + MATRIX_AT + MATRIX_LEN,
        id: file.table_row(index).and_then(|row| row.id),
    }
}

fn read_record(data: &[u8], observe: impl FnOnce(usize)) -> Result<Raw, CorruptChunk> {
    let mut reader = ByteReader::new(data);
    let name = reader.read_bytes(NAME_LEN)?;
    let name = name.split(|&byte| byte == 0).next().unwrap_or(name);
    let object = reader.read_u32()?;
    let parent = reader.read_u32()?;
    reader.skip(UNUSED_WORDS)?;
    let mut matrix = [[0.0_f32; 4]; 4];
    for value in matrix.iter_mut().flatten() {
        *value = reader.read_f32()?;
    }
    if matrix.iter().flatten().any(|value| !value.is_finite()) {
        return Err(CorruptChunk::NonFinite("node transform"));
    }
    observe(name.len());
    Ok(Raw {
        object,
        parent,
        name: String::from_utf8_lossy(name).into_owned(),
        local: local_transform(&matrix),
    })
}

/// Converts the row-vector matrix of a node, translation in the last row, to a [`Transform`];
/// an all-zero matrix is the identity.
fn local_transform(matrix: &[[f32; 4]; 4]) -> Transform {
    if matrix.iter().flatten().all(|&value| value == 0.0) {
        return Transform::IDENTITY;
    }
    let at = |row: usize, column: usize| {
        matrix
            .get(row)
            .and_then(|values| values.get(column))
            .copied()
            .unwrap_or(0.0)
    };
    Transform {
        rows: std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                if column == 3 {
                    at(3, row)
                } else {
                    at(column, row)
                }
            })
        }),
    }
}
