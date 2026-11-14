// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DecimalU64, FieldOrigin, OriginArtifactDigest, PhysicalSource, SourceUseDescription,
    SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

pub(crate) const FORMAT: &str = "p4kx-mesh-vertex-map";
pub(crate) const VERSION: u8 = 2;
pub(crate) const PACKAGE_VERSION: &str = "1.20";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(super) enum Row<'a> {
    Header {
        #[schemars(regex(pattern = "^p4kx-mesh-vertex-map$"))]
        format: &'static str,
        #[schemars(range(min = 1, max = 2))]
        version: u8,
    },
    Artifact {
        glb: &'a str,
        artifact: &'a OriginArtifactDigest,
    },
    Mesh {
        mesh: usize,
        position_accessor: usize,
        vertices: DecimalU64,
        model_node: usize,
        requested_lod: u32,
        selected_lod: u32,
        geometry_lod: Option<u32>,
        geometry_chunk: Option<usize>,
        mapping: Mapping,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
    },
    #[schemars(extend("required" = [
        "type", "mesh", "primitive", "indexAccessor", "materialIndex", "materialSource",
        "modelSubmesh", "sourceSubmesh",
    ]))]
    Primitive {
        mesh: usize,
        primitive: usize,
        index_accessor: Option<usize>,
        material_index: Option<usize>,
        material_source: RendererMaterial,
        model_submesh: usize,
        source_submesh: Option<&'a SourceSubmesh>,
    },
    Source {
        source: &'a PhysicalSource,
    },
    SourceUse {
        source_use: &'a SourceUseDescription,
    },
    Vertex {
        mesh: usize,
        vertex: u32,
        source_vertex: u32,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
    },
    End {
        meshes: DecimalU64,
        vertices: DecimalU64,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(extend("not" = {"type": "null"}))]
        primitives: Option<DecimalU64>,
        sources: DecimalU64,
        source_uses: DecimalU64,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum Mapping {
    Present,
    Unknown,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum RendererMaterial {
    Library { slot: usize },
    SuppliedFallback,
    BuiltInFallback,
    Unassigned,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceSubmesh {
    #[schemars(extend("properties" = {"state": {"const": "present"}}))]
    pub(super) row: SourcedValue<usize>,
    #[schemars(extend("properties" = {"state": {"const": "present"}}))]
    pub(super) node: SourcedValue<u16>,
    #[schemars(extend("properties" = {"state": {"const": "present"}}))]
    pub(super) material: SourcedValue<u16>,
}

/// Returns the schema of one streamed Blender mesh vertex map row.
#[must_use]
pub fn mesh_vertex_map_schema() -> serde_json::Value {
    schemars::schema_for!(Row<'static>).to_value()
}
