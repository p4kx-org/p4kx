// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataType;

pub(crate) struct Original {
    pub(crate) configurations: Vec<Configuration>,
    pub(crate) visited: u64,
}

pub(crate) struct Configuration {
    pub(crate) declaration_index: Option<usize>,
    pub(crate) declaration: Option<Declaration>,
    pub(crate) nodes: Vec<Node>,
}

pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) data_type: DataType,
    pub(crate) is_array: bool,
    pub(crate) target: u16,
}

pub(crate) struct Node {
    pub(crate) ordinal: u64,
    pub(crate) declaration_ordinal: u64,
    pub(crate) parent: Option<usize>,
    pub(crate) edge: Edge,
    pub(crate) kind: SourceKind,
    pub(crate) fields: Vec<Declaration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Root,
    Field(usize),
    Element(usize),
}

pub(crate) enum SourceKind {
    Struct(String),
    Array,
    Null,
    Scalar(&'static str),
    InvalidPointer {
        struct_index: u32,
        instance_index: u32,
    },
}
