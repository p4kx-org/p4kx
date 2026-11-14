// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_foundation::ByteReader;

use crate::hierarchy::find_cycle;
use crate::model_node_coordinates::MatrixRead;
use crate::nmc_layout::{HEADER_BYTES, NmcLayout, name_line};
use crate::nmc_properties;
use crate::reader::ReadValues;
use crate::{
    CorruptChunk, ModelFile, ModelNodePropertySource, ModelNodeSelection, ModelNodeSource,
    ModelNodeSources, Node, Transform,
};

pub(crate) const NODE_LEN: usize = 208;
pub(crate) const LOCAL_TRANSFORM_AT: usize = 80;
pub(crate) const SCALE_AT: usize = LOCAL_TRANSFORM_AT + 48;
pub(crate) const PARENT_AT: usize = 148;
pub(crate) const GEOMETRY_TYPE_AT: usize = PARENT_AT + size_of::<u16>();

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
    mut sources: Option<&mut ModelNodeSources>,
) -> Result<Option<Nmc>, NodeDecodeError> {
    let selected = file.chunks_of(ivo::NODE_MESH_COMBO).next();
    if let Some(sources) = sources.as_mut() {
        sources.set_selection(ModelNodeSelection::ivo(
            file,
            selected.map(|(index, chunk)| (index, chunk.offset())),
        ));
    }
    let Some((index, chunk)) = selected else {
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
    let layout = NmcLayout::read(data)?;
    let material_indices = layout
        .material_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .collect();
    if let Some(sources) = sources.as_mut() {
        sources.start(layout.names_range(), layout.properties_range());
    }
    let nodes = read_rows(layout.records, layout.names, layout.properties, sources)?;
    let parents: Vec<Option<usize>> = nodes.iter().map(|node| node.parent).collect();
    if let Some(node) = find_cycle(&parents) {
        return Err(CorruptChunk::ParentCycle(node));
    }
    Ok(Nmc {
        nodes,
        material_indices,
    })
}

fn read_rows(
    records: &[u8],
    name_bytes: &[u8],
    property_bytes: &[u8],
    mut sources: Option<NodeMeshSources<'_>>,
) -> Result<Vec<Node>, CorruptChunk> {
    let count = records.len() / NODE_LEN;
    let mut names = name_bytes.split(|&byte| byte == 0);
    let mut properties = property_bytes.split(|&byte| byte == 0);

    let mut nodes = Vec::with_capacity(count);
    for (index, record) in records.as_chunks::<NODE_LEN>().0.iter().enumerate() {
        let mut record = read_record(index, record, count)?;
        let name = names.next();
        let line = name.map(name_line);
        let entry = properties.next();
        record.node.name = line.map_or_else(String::new, |bytes| {
            String::from_utf8_lossy(bytes).into_owned()
        });
        let props = nmc_properties::read_entry(entry, sources.as_ref().map(|s| s.properties.start));
        record.node.properties = props.values;
        if let Some(sources) = sources.as_mut() {
            sources.record(
                index,
                &record,
                NodeStrings {
                    name: name.map(<[u8]>::len),
                    line: line.map(<[u8]>::len),
                    property: entry.map(<[u8]>::len),
                },
                props.sites,
            );
        }
        nodes.push(record.node);
    }
    Ok(nodes)
}

struct Record {
    node: Node,
    matrix_read: MatrixRead,
    scale_read: bool,
}

fn read_record(
    index: usize,
    record: &[u8; NODE_LEN],
    count: usize,
) -> Result<Record, CorruptChunk> {
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
    let (transform, matrix_read) = if rows.iter().flatten().all(|value| value.abs() < EPSILON) {
        (Transform::IDENTITY, MatrixRead::Identity)
    } else {
        (Transform { rows }, MatrixRead::Rows)
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
    let scale_read = scale.iter().any(|value| value.abs() >= EPSILON);
    if scale_read {
        node.scale = scale;
    }
    node.geometry_type = Some(geometry_type);
    Ok(Record {
        node,
        matrix_read,
        scale_read,
    })
}

#[derive(Clone, Copy)]
struct NodeStrings {
    name: Option<usize>,
    line: Option<usize>,
    property: Option<usize>,
}

struct NodeMeshSources<'a> {
    nodes: &'a mut ModelNodeSources,
    chunk: usize,
    base: u64,
    records: u64,
    names: Range<u64>,
    properties: Range<u64>,
}

impl<'a> NodeMeshSources<'a> {
    const fn new(nodes: &'a mut ModelNodeSources, chunk: usize, base: u64) -> Self {
        Self {
            nodes,
            chunk,
            base,
            records: 0,
            names: 0..0,
            properties: 0..0,
        }
    }

    const fn start(&mut self, names: Range<u64>, properties: Range<u64>) {
        self.records = self.base + HEADER_BYTES as u64;
        self.names = self.base + names.start..self.base + names.end;
        self.properties = self.base + properties.start..self.base + properties.end;
    }

    fn record(
        &mut self,
        index: usize,
        record: &Record,
        strings: NodeStrings,
        property_rows: Vec<ModelNodePropertySource>,
    ) {
        let start = self.records + index as u64 * NODE_LEN as u64;
        let name = text_extent(&self.names, strings.line);
        if let Some(extent) = text_extent(&self.names, strings.name) {
            self.names.start = extent.end;
        }
        let properties = text_extent(&self.properties, strings.property);
        if let Some(extent) = &properties {
            self.properties.start = extent.end;
        }
        self.nodes.push(ModelNodeSource {
            chunk: self.chunk,
            name,
            parent: start + PARENT_AT as u64..start + PARENT_AT as u64 + 2,
            matrix: start + LOCAL_TRANSFORM_AT as u64..start + SCALE_AT as u64,
            table: None,
            matrix_read: record.matrix_read,
            scale: Some(start + SCALE_AT as u64..start + SCALE_AT as u64 + 12),
            scale_read: record.scale_read,
            geometry_type: Some(
                start + GEOMETRY_TYPE_AT as u64..start + GEOMETRY_TYPE_AT as u64 + 2,
            ),
            properties,
            property_rows,
        });
    }
}

fn text_extent(table: &Range<u64>, length: Option<usize>) -> Option<Range<u64>> {
    let end = table.start + length? as u64;
    let with_separator = (end + 1).min(table.end);
    (with_separator > table.start).then_some(table.start..with_separator)
}
