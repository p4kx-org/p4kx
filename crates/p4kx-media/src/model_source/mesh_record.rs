// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::SourceRange;

pub(super) const MESH: &str = "model-mesh";
pub(super) const STREAM: &str = "model-mesh-stream";
pub(super) const SUBMESH: &str = "model-mesh-submesh";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Mesh {
    Measured {
        vertices: u32,
        indices: u32,
        submeshes: u32,
        extra_words: u32,
        model_bounds_bits: [[u32; 3]; 2],
        scaling_bounds_bits: [[u32; 3]; 2],
    },
    UnsupportedVersion,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Stream {
    pub(super) tag: Option<u32>,
    pub(super) stride_bytes: Option<u32>,
    pub(super) count: Option<u32>,
    pub(super) count_basis: Option<CountBasis>,
    pub(super) payload: Option<SourceRange>,
    pub(super) padding: Option<SourceRange>,
    pub(super) retained_tail: Option<SourceRange>,
    pub(super) disposition: StreamDisposition,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CountBasis {
    VertexCount,
    IndexCount,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum StreamDisposition {
    Bounded,
    OpaqueTail,
    Incomplete,
    MalformedHeader,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Submesh {
    pub(super) material_id: u16,
    pub(super) node_id: u16,
    pub(super) first_index: u32,
    pub(super) index_count: u32,
    pub(super) vertex_page_base: u32,
    pub(super) source_vertices: VertexMapping,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum VertexMapping {
    StoredIndices {
        stream: usize,
        stride_bytes: u32,
        indices: SourceRange,
    },
    NoIndices,
    MissingIndexStream,
    AmbiguousIndexStreams,
    InvalidIndexStream,
    InvalidIndexRange,
    InvalidSourceVertex,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum MeshRecord {
    Mesh(Mesh),
    MeshStream(Stream),
    MeshSubmesh(Submesh),
}

const SCHEMA: &str = "ModelMeshRecord";
pub(super) const SCHEMA_ROWS: [(&str, &str, usize, &str); 3] = [
    (MESH, "mesh", 1, SCHEMA),
    (STREAM, "mesh-stream", 2, SCHEMA),
    (SUBMESH, "mesh-submesh", 2, SCHEMA),
];

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), crate::MediaError> {
    crate::schema_merge::insert_payload(
        base,
        SCHEMA,
        crate::schema_merge::serialized_payload::<MeshRecord>(),
    )
}
