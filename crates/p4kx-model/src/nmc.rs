// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_foundation::ByteReader;

use crate::hierarchy::find_cycle;
use crate::reader::{ReadValues, to_usize};
use crate::{CorruptChunk, ModelFile, ModelNodeSource, ModelNodeSources, Node, Transform};

const NODE_LEN: usize = 208;
const LOCAL_TRANSFORM_AT: usize = 80;
const SCALE_AT: usize = LOCAL_TRANSFORM_AT + 48;
const PARENT_AT: usize = 148;
const FOOTER_LEN: usize = 32;
const ROOT: u16 = 0xFFFF;
const EPSILON: f32 = 1e-5;

/// The contents of a `NodeMeshCombo` chunk the model reads.
#[derive(Debug)]
pub(crate) struct Nmc {
    pub(crate) nodes: Vec<Node>,
    pub(crate) material_indices: Vec<u16>,
}

pub(crate) struct NodeDecodeError {
    pub(crate) index: usize,
    pub(crate) source: CorruptChunk,
}

pub(crate) fn read_file_nodes<B: AsRef<[u8]>>(
    file: &ChunkFile<B>,
    read: &mut Vec<(ModelFile, usize)>,
    sources: Option<&mut ModelNodeSources>,
) -> Result<Option<Nmc>, NodeDecodeError> {
    let Some((index, chunk)) = file.chunks_of(ivo::NODE_MESH_COMBO).next() else {
        return Ok(None);
    };
    read.push((ModelFile::Model, index));
    let data = file.chunk_data(index).unwrap_or_default();
    let sources = sources.map(|nodes| NodeMeshSources::new(nodes, index, chunk.offset()));
    let nodes = read_nodes(data, sources).map_err(|source| NodeDecodeError { index, source })?;
    Ok((!nodes.nodes.is_empty()).then_some(nodes))
}

/// Reads a `NodeMeshCombo` chunk: a header of eight words, one 208-byte record per node, a
/// footer, two index lists, the table of node names and the table of node properties. Both
/// tables hold one NUL-terminated entry per node; an entry of the second is `key = value` lines
/// ending in CR LF, e.g. `class=AnimatedJoint`.
///
/// A node's transform and scale that are zero on every axis (within [`EPSILON`]) are the
/// identity and 1: records that store none.
fn read_nodes(data: &[u8], mut sources: Option<NodeMeshSources<'_>>) -> Result<Nmc, CorruptChunk> {
    let mut reader = ByteReader::new(data);
    reader.read_u32()?;
    let count = to_usize(reader.read_u32()?);
    reader.read_u32()?;
    let other_indices = to_usize(reader.read_u32()?);
    let material_indices = to_usize(reader.read_u32()?);
    let names_len = to_usize(reader.read_u32()?);
    let properties_len = to_usize(reader.read_u32()?);
    reader.skip(4)?;

    let records_at = reader.offset();
    let records = reader.read_bytes(count.saturating_mul(NODE_LEN))?;
    reader.skip(FOOTER_LEN)?;
    reader.skip(other_indices.saturating_mul(2))?;
    let material_bytes = reader.read_bytes(material_indices.saturating_mul(2))?;
    let material_indices = material_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .collect();
    if let Some(sources) = sources.as_mut() {
        sources.start(records_at, reader.offset(), names_len);
    }
    let names = reader.read_bytes(names_len)?;
    let mut names = names.split(|&byte| byte == 0);
    let properties = reader.read_bytes(properties_len.min(reader.remaining()))?;
    let mut properties = properties.split(|&byte| byte == 0);

    let mut nodes = Vec::with_capacity(count);
    for (index, record) in records.as_chunks::<NODE_LEN>().0.iter().enumerate() {
        let mut node = read_record(index, record, count)?;
        let name = names.next();
        let line = name.and_then(|bytes| bytes.split(|&byte| byte == b'\n').next());
        node.name = line.map_or_else(String::new, |bytes| {
            String::from_utf8_lossy(bytes).into_owned()
        });
        if let Some(sources) = sources.as_mut() {
            sources.record(index, name.map(<[u8]>::len), line.map(<[u8]>::len));
        }
        node.properties = properties
            .next()
            .map(|entry| {
                entry
                    .split(|&byte| byte == b'\n')
                    .filter_map(property)
                    .collect()
            })
            .unwrap_or_default();
        nodes.push(node);
    }
    let parents: Vec<Option<usize>> = nodes.iter().map(|node| node.parent).collect();
    if let Some(node) = find_cycle(&parents) {
        return Err(CorruptChunk::ParentCycle(node));
    }
    Ok(Nmc {
        nodes,
        material_indices,
    })
}

fn read_record(index: usize, record: &[u8; NODE_LEN], count: usize) -> Result<Node, CorruptChunk> {
    let mut fields = ByteReader::new(record.as_slice());
    fields.skip(LOCAL_TRANSFORM_AT)?;
    let rows = fields.read_rows()?;
    let scale = [fields.read_f32()?, fields.read_f32()?, fields.read_f32()?];
    fields.skip(PARENT_AT - SCALE_AT - 12)?;
    let parent = fields.read_u16()?;
    let geometry_type = fields.read_u16()?;
    if rows.iter().flatten().any(|value| !value.is_finite()) {
        return Err(CorruptChunk::NonFinite("node transform"));
    }
    if scale.iter().any(|value| !value.is_finite()) {
        return Err(CorruptChunk::NonFinite("node scale"));
    }
    let transform = if rows.iter().flatten().all(|value| value.abs() < EPSILON) {
        Transform::IDENTITY
    } else {
        Transform { rows }
    };
    let parent = match parent {
        ROOT => None,
        parent if usize::from(parent) < count => Some(usize::from(parent)),
        parent => {
            return Err(CorruptChunk::ParentRange {
                node: index,
                parent,
                nodes: count,
            });
        }
    };
    let mut node = Node::new(String::new(), parent, transform);
    if scale.iter().any(|value| value.abs() >= EPSILON) {
        node.scale = scale;
    }
    node.geometry_type = Some(geometry_type);
    Ok(node)
}

fn property(line: &[u8]) -> Option<(String, String)> {
    let at = line.iter().position(|&byte| byte == b'=')?;
    let (key, value) = line.split_at(at);
    let key = key.trim_ascii();
    let value = value.get(1..).unwrap_or_default().trim_ascii();
    (!key.is_empty()).then(|| {
        (
            String::from_utf8_lossy(key).into_owned(),
            String::from_utf8_lossy(value).into_owned(),
        )
    })
}

struct NodeMeshSources<'a> {
    nodes: &'a mut ModelNodeSources,
    chunk: usize,
    base: u64,
    records: u64,
    names: Range<u64>,
}

impl<'a> NodeMeshSources<'a> {
    const fn new(nodes: &'a mut ModelNodeSources, chunk: usize, base: u64) -> Self {
        Self {
            nodes,
            chunk,
            base,
            records: 0,
            names: 0..0,
        }
    }

    const fn start(&mut self, records: u64, names: u64, names_len: usize) {
        self.records = self.base + records;
        self.names = self.base + names..self.base + names + names_len as u64;
    }

    fn record(&mut self, index: usize, name_len: Option<usize>, line_len: Option<usize>) {
        let start = self.records + index as u64 * NODE_LEN as u64;
        let name = line_len.and_then(|len| {
            let end = self.names.start + len as u64;
            let with_terminator = if end < self.names.end { end + 1 } else { end };
            (with_terminator > self.names.start).then_some(self.names.start..with_terminator)
        });
        if let Some(len) = name_len {
            self.names.start = (self.names.start + len as u64 + 1).min(self.names.end);
        }
        self.nodes.push(ModelNodeSource {
            chunk: self.chunk,
            name,
            parent: start + PARENT_AT as u64..start + PARENT_AT as u64 + 2,
            matrix: start + LOCAL_TRANSFORM_AT as u64..start + SCALE_AT as u64,
            id: None,
        });
    }
}
