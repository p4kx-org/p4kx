// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ChunkJson, FieldOrigins, SourceInventory};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// The chunks of a chunk file.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ChunksResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path of the chunk file.
    pub(crate) path: String,
    /// The container form: `#ivo` or `CrCh`.
    pub(crate) container: &'static str,
    /// The container version.
    pub(crate) container_version: u32,
    /// The number of chunks in the file.
    pub(crate) chunk_count: usize,
    /// The number of chunks in the file whose type is not in the table of known types.
    pub(crate) unknown_count: usize,
    /// The number of chunks listed: all of them, or those of the requested `type`.
    pub(crate) total: usize,
    /// The chunks on this page in table order.
    pub(crate) chunks: Vec<ChunkShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    /// The original inputs of each structured output field.
    #[serde(rename = "fieldOrigins")]
    pub(crate) field_origins: FieldOrigins,
    /// The physical sources and exact uses referenced by the fields.
    pub(crate) provenance: SourceInventory,
}

/// One chunk of a chunk file.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ChunkShape {
    /// The chunk as `p4kx chunks list --json` shows it.
    #[serde(flatten)]
    pub(crate) chunk: ChunkJson,
    /// The type ID in hexadecimal.
    pub(crate) type_id_hex: String,
}

/// A model summarised: what `p4kx model info --json` says, with the long lists left to
/// `list_model_items`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ModelOverview {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path of the model.
    pub(crate) model: String,
    /// The geometry companion the model names.
    pub(crate) geometry: Option<String>,
    /// The material library the model names.
    pub(crate) material_library: Option<String>,
    /// The material IDs the meshes use.
    pub(crate) material_ids: Vec<u16>,
    /// What reading the material library came to: `path`, `status` (`loaded`, `missing`,
    /// `unreadable` or `unnamed`), `problem`, `single` and `default_palette`.
    #[schemars(with = "Value")]
    pub(crate) library: Value,
    /// How many items each section of `list_model_items` has.
    pub(crate) counts: ModelCounts,
    /// The levels of detail with their size.
    pub(crate) lods: Vec<LodShape>,
}

/// The number of items in each section of a model.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ModelCounts {
    /// The materials of the library.
    pub(crate) materials: usize,
    /// The nodes.
    pub(crate) nodes: usize,
    /// The levels of detail.
    pub(crate) lods: usize,
    /// The meshes of all levels of detail.
    pub(crate) meshes: usize,
    /// The bones.
    pub(crate) bones: usize,
    /// The animations read.
    pub(crate) animations: usize,
    /// The animation files and animations that could not be read.
    pub(crate) unread_animations: usize,
    /// The chunks of the model's files.
    pub(crate) chunks: usize,
}

/// One level of detail of a model.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LodShape {
    /// The level, 0 for the finest.
    pub(crate) level: u32,
    /// The vertices of all its meshes.
    pub(crate) vertices: usize,
    /// The triangles of all its meshes.
    pub(crate) triangles: usize,
    /// The number of meshes.
    pub(crate) meshes: usize,
}

/// One page of a section of a model.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ModelItemsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path of the model.
    pub(crate) model: String,
    /// The section listed.
    pub(crate) section: &'static str,
    /// The number of items in the section.
    pub(crate) total: usize,
    /// The items on this page, each as `p4kx model info --json` shows it in the section of the
    /// same name.
    #[schemars(with = "Vec<Value>")]
    pub(crate) items: Vec<Value>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}
