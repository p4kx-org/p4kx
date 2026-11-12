// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Deserialize;

/// The longest `type` argument accepted, in bytes.
pub(crate) const MAX_TYPE_LEN: usize = 64;

/// The arguments of `list_chunks`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChunksParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entry path of the chunk file, e.g. `Data/Objects/ship.cgf`, a `.cgf`, `.cga`, `.skin`,
    /// `.chr`, `.cgfm`, `.caf` or `.dba`; case and separator kind do not matter.
    pub(crate) path: String,
    /// Lists only the chunks of this type: a type name such as `Mesh` or `CompiledBones`, or a
    /// hexadecimal type ID such as `0x9293B9D8`.
    #[serde(rename = "type")]
    pub(crate) chunk_type: Option<String>,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The arguments of `get_model_info`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entry path of the model, e.g. `Data/Objects/ship.cgf`; case and separator kind do not
    /// matter.
    pub(crate) path: String,
}

/// The parts of a model `list_model_items` lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Section {
    /// The materials of the model's material library.
    Materials,
    /// The nodes with their parent and transform.
    Nodes,
    /// The meshes of all levels of detail, each with its `lod` and submeshes.
    Meshes,
    /// The bones of the skeleton with their poses.
    Bones,
    /// The animations read for the model.
    Animations,
    /// The animation files and animations that could not be read.
    UnreadAnimations,
    /// The chunks of the model's files, with how each was used.
    Chunks,
}

impl Section {
    /// Returns the name the tool arguments and results use.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Materials => "materials",
            Self::Nodes => "nodes",
            Self::Meshes => "meshes",
            Self::Bones => "bones",
            Self::Animations => "animations",
            Self::UnreadAnimations => "unread_animations",
            Self::Chunks => "chunks",
        }
    }
}

/// The arguments of `list_model_items`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelItemsParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entry path of the model, e.g. `Data/Objects/ship.cgf`; case and separator kind do not
    /// matter.
    pub(crate) path: String,
    /// The part of the model to list.
    pub(crate) section: Section,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}
