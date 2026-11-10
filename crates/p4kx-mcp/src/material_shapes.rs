// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The arguments of `mtl_summary`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MaterialParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entry path of the `.mtl` file, e.g. `Data/Objects/ship.mtl`; case and separator kind
    /// do not matter. An Entry inside a container is `outer.socpak::path`.
    pub(crate) path: String,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size in sub-materials, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The sub-materials of one `.mtl` file.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct MaterialsResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entry path of the `.mtl` file.
    pub(crate) path: String,
    /// Whether the file is one material that serves every material ID, rather than a list of
    /// sub-materials.
    pub(crate) single: bool,
    /// The tint palette the file names as its default, a record path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) default_palette: Option<String>,
    /// The number of sub-material list entries that are no material and read as the engine's
    /// default material.
    pub(crate) unknown_entries: usize,
    /// The number of sub-materials.
    pub(crate) total: usize,
    /// The sub-materials on this page, in material ID order.
    pub(crate) materials: Vec<MaterialShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// One sub-material.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct MaterialShape {
    /// The material ID: the position in the list.
    pub(crate) id: usize,
    /// The material's name.
    pub(crate) name: String,
    /// The shader, e.g. `Illum`.
    pub(crate) shader: String,
    /// The physical surface type.
    pub(crate) surface_type: String,
    /// The shader generation mask.
    pub(crate) string_gen_mask: String,
    /// The engine flags.
    pub(crate) flags: u32,
    /// Whether the engine draws both faces of a triangle.
    pub(crate) two_sided: bool,
    /// Whether the engine draws nothing with the material.
    pub(crate) no_draw: bool,
    /// The linear diffuse colour.
    pub(crate) diffuse: [f32; 3],
    /// The linear specular colour.
    pub(crate) specular: [f32; 3],
    /// The linear emissive colour.
    pub(crate) emissive: [f32; 3],
    /// The glossiness from 0 to 255.
    pub(crate) shininess: f32,
    /// The opacity from 0 to 1.
    pub(crate) opacity: f32,
    /// The alpha-test threshold, 0 when the material is not alpha tested.
    pub(crate) alpha_test: f32,
    /// The glow amount the emissive colour is multiplied by.
    pub(crate) glow: f32,
    /// The palette colour the material takes, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) palette: Option<String>,
    /// The texture slots in file order, at most 64.
    pub(crate) textures: Vec<TextureShape>,
    /// The number of texture slots in the file.
    pub(crate) texture_count: usize,
    /// The shader parameters in file order, at most 64.
    pub(crate) public_params: Vec<ParamShape>,
    /// The number of shader parameters in the file.
    pub(crate) public_param_count: usize,
    /// The material layers in file order, at most 64.
    pub(crate) layers: Vec<LayerShape>,
    /// The number of material layers in the file.
    pub(crate) layer_count: usize,
}

/// One texture slot.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TextureShape {
    /// The slot, e.g. `TexSlot1`.
    pub(crate) map: String,
    /// The texture path below `Data`, or an engine placeholder.
    pub(crate) file: String,
}

/// One shader parameter.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ParamShape {
    /// The parameter name.
    pub(crate) name: String,
    /// The value as written.
    pub(crate) value: String,
}

/// One material layer.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LayerShape {
    /// The layer's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    /// The path of the layer's own `.mtl` file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    /// The `PaletteTint` as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) palette_tint: Option<String>,
}
