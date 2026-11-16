// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{DataForge, DataForgeOrigin, Guid, Record, SourceEvidenceError, Struct};
use rayon::slice::ParallelSliceMut;

use crate::sort::compare_ignoring_case;
use crate::{
    DataForgeSummaryProvenance,
    dataforge_summary_provenance::{Capture, ReadIdentity},
};

/// The number of Records the DataForge summary lists.
pub const SUMMARY_RECORDS: usize = 10;

/// Identifies a node of the Record tree; the DataForge node itself is the root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordNodeId(pub(crate) u32);

/// Identifies a Record of a loaded DataForge by its position in definition order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordId(pub(crate) u32);

/// What a node of the Record tree stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordNodeKind {
    /// A folder of record paths, or the DataForge node itself.
    Folder,
    /// A record path that several Records share; its children are those Records.
    RecordFolder,
    /// One Record.
    Record(RecordId),
}

/// One Record as the tree shows it: name, type, record path and GUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordInfo {
    name: Box<str>,
    type_name: Arc<str>,
    record_path: Box<str>,
    guid: Guid,
}

impl RecordInfo {
    /// Returns the name, e.g. `EntityClassDefinition.AEGS_Avenger`, or the GUID when the name
    /// cannot be read.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the name of the Record's struct, empty when it cannot be read.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the record path, empty when it cannot be read.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the GUID.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }
}

/// The facts the DataForge tab of the preview shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataForgeSummary {
    version: u32,
    record_count: usize,
    struct_count: usize,
    enum_count: usize,
    first_records: Vec<RecordInfo>,
    provenance: Result<Box<DataForgeSummaryProvenance>, SourceEvidenceError>,
}

impl DataForgeSummary {
    /// Returns the version from the header.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the number of Records.
    #[must_use]
    pub const fn record_count(&self) -> usize {
        self.record_count
    }

    /// Returns the number of struct definitions.
    #[must_use]
    pub const fn struct_count(&self) -> usize {
        self.struct_count
    }

    /// Returns the number of enum definitions.
    #[must_use]
    pub const fn enum_count(&self) -> usize {
        self.enum_count
    }

    /// Returns the first [`SUMMARY_RECORDS`] Records in definition order.
    #[must_use]
    pub fn first_records(&self) -> &[RecordInfo] {
        &self.first_records
    }

    /// Returns the original sources of the facts and first Record cells.
    #[must_use]
    pub fn provenance(&self) -> Result<&DataForgeSummaryProvenance, SourceEvidenceError> {
        self.provenance.as_deref().map_err(|error| *error)
    }
}

#[derive(Debug)]
pub(crate) struct Node {
    pub(crate) name: Box<str>,
    pub(crate) kind: RecordNodeKind,
    pub(crate) children: Vec<RecordNodeId>,
    pub(crate) parent: Option<RecordNodeId>,
}

/// Every Record of a DataForge with the folder tree of their record paths.
#[derive(Debug)]
pub(crate) struct RecordIndex {
    pub(crate) records: Vec<RecordInfo>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) summary: DataForgeSummary,
    pub(crate) by_path: Vec<RecordId>,
    pub(crate) record_nodes: Vec<RecordNodeId>,
}

impl RecordIndex {
    /// Reads every Record of `forge` and builds the tree; this belongs on a background thread.
    pub(crate) fn build(forge: &DataForge<'_>, origin: Option<&DataForgeOrigin>) -> Self {
        let mut types = HashMap::new();
        let mut capture = Capture::new(origin);
        let records: Vec<RecordInfo> = forge
            .records()
            .enumerate()
            .map(|(position, record)| {
                let capture = (position < SUMMARY_RECORDS).then_some(&mut capture);
                info(forge, &record, &mut types, capture)
            })
            .collect();
        let summary = DataForgeSummary {
            version: forge.version(),
            record_count: forge.record_count(),
            struct_count: forge.struct_count(),
            enum_count: forge.enum_count(),
            first_records: records.iter().take(SUMMARY_RECORDS).cloned().collect(),
            provenance: capture.and_then(Capture::finish).map(Box::new),
        };
        let (nodes, by_path) = tree(&records);
        let mut record_nodes = vec![RecordNodeId(0); records.len()];
        for (node, id) in nodes.iter().zip((0..).map(RecordNodeId)) {
            if let RecordNodeKind::Record(record) = node.kind
                && let Some(slot) = record_nodes.get_mut(record.0 as usize)
            {
                *slot = id;
            }
        }
        Self {
            records,
            nodes,
            summary,
            by_path: by_path.into_iter().map(RecordId).collect(),
            record_nodes,
        }
    }
}

fn info<'f>(
    forge: &DataForge<'_>,
    record: &Record<'f>,
    types: &mut HashMap<&'f str, Arc<str>>,
    capture: Option<&mut Result<Capture, SourceEvidenceError>>,
) -> RecordInfo {
    let guid = record.guid();
    let name = record.name().ok();
    let structure = record.struct_type().ok();
    let type_name = structure
        .as_ref()
        .and_then(|structure| structure.name().ok());
    let path = record.record_path().ok();
    if let Some(capture) = capture
        && let Ok(bound) = capture
        && let Err(error) = bound.row(
            forge,
            ReadIdentity {
                index: record.index(),
                name,
                struct_index: structure.as_ref().map(Struct::index),
                type_name,
                path,
                file_name: path.and_then(|_| record.stored_file_name().ok()),
            },
        )
    {
        *capture = Err(error);
    }
    let type_name = type_name.unwrap_or_default();
    RecordInfo {
        name: name.map_or_else(|| guid.to_string().into(), Into::into),
        type_name: Arc::clone(types.entry(type_name).or_insert_with(|| type_name.into())),
        record_path: path.unwrap_or_default().into(),
        guid,
    }
}

const fn key(byte: u8) -> u8 {
    if byte == b'\\' {
        b'/'
    } else {
        byte.to_ascii_lowercase()
    }
}

fn compare_paths(left: &str, right: &str) -> Ordering {
    left.bytes().map(key).cmp(right.bytes().map(key))
}

fn segments(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect()
}

fn tree(records: &[RecordInfo]) -> (Vec<Node>, Vec<u32>) {
    let mut order: Vec<u32> = (0..records.len())
        .filter_map(|index| u32::try_from(index).ok())
        .collect();
    let at = |index: u32| records.get(index as usize);
    order.par_sort_unstable_by(|&a, &b| match (at(a), at(b)) {
        (Some(left), Some(right)) => compare_paths(&left.record_path, &right.record_path)
            .then_with(|| compare_ignoring_case(&left.name, &right.name))
            .then(a.cmp(&b)),
        _ => a.cmp(&b),
    });
    let mut builder = Builder {
        nodes: vec![folder("")],
        open: vec![0],
    };
    let groups = order.chunk_by(|&a, &b| {
        at(a)
            .zip(at(b))
            .is_some_and(|(l, r)| compare_paths(&l.record_path, &r.record_path).is_eq())
    });
    for group in groups {
        let Some(first) = group.first().and_then(|&index| at(index)) else {
            continue;
        };
        let parts = segments(&first.record_path);
        let (file, dirs) = parts
            .split_last()
            .map_or((None, &[][..]), |(f, d)| (Some(*f), d));
        let parent = builder.enter(dirs);
        builder.add_group(parent, file, group, records);
    }
    sort_children(&mut builder.nodes);
    (builder.nodes, order)
}

fn folder(name: &str) -> Node {
    Node {
        name: name.into(),
        kind: RecordNodeKind::Folder,
        children: Vec::new(),
        parent: None,
    }
}

struct Builder {
    nodes: Vec<Node>,
    open: Vec<u32>,
}

impl Builder {
    fn enter(&mut self, dirs: &[&str]) -> u32 {
        let kept = self
            .open
            .iter()
            .skip(1)
            .zip(dirs)
            .take_while(|&(&node, dir)| {
                self.nodes
                    .get(node as usize)
                    .is_some_and(|node| node.name.eq_ignore_ascii_case(dir))
            })
            .count();
        self.open.truncate(kept + 1);
        for dir in dirs.iter().skip(kept) {
            let parent = self.open.last().copied().unwrap_or_default();
            let Some(id) = self.push(parent, folder(dir)) else {
                break;
            };
            self.open.push(id);
        }
        self.open.last().copied().unwrap_or_default()
    }

    fn push(&mut self, parent: u32, node: Node) -> Option<u32> {
        let id = u32::try_from(self.nodes.len()).ok()?;
        if let Some(parent) = self.nodes.get_mut(parent as usize) {
            parent.children.push(RecordNodeId(id));
        }
        self.nodes.push(Node {
            parent: Some(RecordNodeId(parent)),
            ..node
        });
        Some(id)
    }

    fn add_group(
        &mut self,
        parent: u32,
        file: Option<&str>,
        group: &[u32],
        records: &[RecordInfo],
    ) {
        let record = |index: u32, name: &str| Node {
            name: name.into(),
            kind: RecordNodeKind::Record(RecordId(index)),
            children: Vec::new(),
            parent: None,
        };
        let name_of = |index: u32| records.get(index as usize).map_or("", |info| &*info.name);
        match (file, group) {
            (Some(file), &[only]) => {
                self.push(parent, record(only, file));
            }
            (Some(file), _) => {
                let Some(shared) = self.push(
                    parent,
                    Node {
                        kind: RecordNodeKind::RecordFolder,
                        ..folder(file)
                    },
                ) else {
                    return;
                };
                for &index in group {
                    self.push(shared, record(index, name_of(index)));
                }
            }
            (None, _) => {
                for &index in group {
                    self.push(parent, record(index, name_of(index)));
                }
            }
        }
    }
}

fn sort_children(nodes: &mut [Node]) {
    for position in 0..nodes.len() {
        let Some(node) = nodes.get_mut(position) else {
            continue;
        };
        let mut children = std::mem::take(&mut node.children);
        let key = |id: &RecordNodeId| {
            nodes
                .get(id.0 as usize)
                .map(|node| (matches!(node.kind, RecordNodeKind::Record(_)), &*node.name))
        };
        children.sort_by(|a, b| match (key(a), key(b)) {
            (Some((a_leaf, a_name)), Some((b_leaf, b_name))) => a_leaf
                .cmp(&b_leaf)
                .then_with(|| compare_ignoring_case(a_name, b_name)),
            _ => a.cmp(b),
        });
        if let Some(node) = nodes.get_mut(position) {
            node.children = children;
        }
    }
}
