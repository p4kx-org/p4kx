// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::manifest_light::Light;
use crate::manifest_materials::{MaterialDescription, Palette};
use crate::manifest_rig::Rig;

/// The value of [`Manifest::format`] as text.
pub const FORMAT: &str = "p4kx-blender-package";

/// The version of the package format this crate writes, `<major>.<minor>`.
///
/// A new minor version only adds fields and values readers may ignore; a new major version changes
/// the meaning of existing ones.
pub const FORMAT_VERSION: &str = "1.7";

/// The name of the manifest file in the package folder.
pub const MANIFEST_FILE: &str = "scene.json";

/// The folder of the GLB meshes in the package folder.
pub const MESHES_FOLDER: &str = "meshes";

/// The folder of the PNG textures in the package folder.
pub const TEXTURES_FOLDER: &str = "textures";

/// The format name in `scene.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Format {
    /// A package of this tool.
    #[serde(rename = "p4kx-blender-package")]
    P4kxBlenderPackage,
}

/// The axes of every transform and of the GLB meshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Basis {
    /// glTF's axes, Y up. A reader refuses a basis it does not know.
    #[serde(rename = "gltf_y_up")]
    GltfYUp,
}

/// The unit of every length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Unit {
    /// Metres. A reader refuses a unit it does not know.
    #[serde(rename = "metre")]
    Metre,
}

/// The image format of a texture file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TextureFormat {
    /// An 8-bit RGBA PNG.
    #[serde(rename = "png")]
    Png,
}

/// How the colour channels of a texture are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColourSpace {
    /// sRGB transfer curve: colour data, such as a diffuse map.
    Srgb,
    /// Linear values: data such as normals, smoothness and metalness.
    Linear,
}

/// What the alpha channel of a texture holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMeaning {
    /// Nothing: the channel is fully opaque and is to be ignored.
    None,
    /// Opacity.
    Opacity,
    /// Smoothness, as in the alpha channel of a `_ddna` normal map.
    Smoothness,
}

/// What a node places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// An Entity of the Archive; `entity` names it.
    Entity,
    /// A group of the Interior: the node that holds the containers of an Entity, a container or a
    /// `.soc` file a container streams. It draws nothing.
    Container,
    /// A model an Interior places, a prop or a placed model; it draws a mesh.
    Object,
    /// A light an Interior places; `light` describes it.
    Light,
    /// A Character: the node draws a mesh whose `rig` holds a skeleton, skinned parts, morph
    /// targets and animations.
    Character,
}

/// What reported an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueOrigin {
    /// Resolving the Assembly.
    Assembly,
    /// Loading or writing a texture.
    Texture,
    /// Reading a material or a material layer.
    Material,
}

/// The scene description of a package: `scene.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    /// Always `p4kx-blender-package`; tells a reader that the folder is a package.
    pub format: Format,
    /// The version of the package format, `<major>.<minor>`, see [`FORMAT_VERSION`]. A reader
    /// refuses a higher major version and warns about a higher minor one.
    #[schemars(regex(pattern = r"^[0-9]+\.[0-9]+$"))]
    pub version: String,
    /// The program that wrote the package.
    pub generator: Generator,
    /// Where the scene comes from.
    pub source: Source,
    /// The axes and unit of every transform and of the GLB meshes.
    pub coordinates: Coordinates,
    /// The choices the package was written with.
    pub options: Options,
    /// The scene hierarchy; a node comes after its parent.
    pub nodes: Vec<Node>,
    /// The GLB meshes, one per distinct mesh set; nodes reference them.
    pub meshes: Vec<Mesh>,
    /// The PNG textures, each written once.
    pub textures: Vec<Texture>,
    /// The material descriptions of the meshes and of their layers, each written once; absent in
    /// version 1.0.
    #[serde(default)]
    pub materials: Vec<MaterialDescription>,
    /// The tint palettes of the meshes, each written once; absent in version 1.0.
    #[serde(default)]
    pub palettes: Vec<Palette>,
    /// What could not be resolved or written, with the node or texture it concerns.
    pub issues: Vec<Issue>,
}

/// The program that wrote a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Generator {
    /// The program's name, `p4kx`.
    pub name: String,
    /// The program's version.
    pub version: String,
}

/// Where a package comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    /// The Channel folder name of the Archive, e.g. `LIVE`; absent when the Archive is in no
    /// Channel folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// The Build of the Archive; absent when its manifest could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildInfo>,
    /// The Entity the scene was built from. A Character package puts the Character's name in
    /// `class_name` and an empty `guid`.
    pub entity: EntityRef,
    /// What a Character was made from (1.3); present in a Character package only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<CharacterSource>,
}

/// What a Character was made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CharacterSource {
    /// The Character's name, also the name of its node.
    pub name: String,
    /// The code of the body variant, e.g. `male`.
    pub body: String,
    /// The CHF file the head came from, as a file name without its folder; absent without a CHF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chf: Option<String>,
    /// The Loadout the pieces came from, as the user named it; absent without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loadout: Option<String>,
}

/// The Build of the Archive a package was made from, as its build manifest states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuildInfo {
    /// The branch.
    pub branch: String,
    /// The game version.
    pub version: String,
    /// The public patch number, when the manifest has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_patch: Option<String>,
    /// The change number.
    pub change_number: String,
    /// The build ID.
    pub build_id: String,
}

/// An Entity of the Archive. In `source.entity` of a Character package, which is made from no
/// Entity, `class_name` is the Character's name and `guid` is empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntityRef {
    /// The Entity's class name, e.g. `AEGS_Avenger_Titan`.
    pub class_name: String,
    /// The Entity's GUID.
    pub guid: String,
}

/// The coordinate system of a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Coordinates {
    /// The axes: always `gltf_y_up`, glTF's, as the GLB meshes have them. Node transforms are
    /// already converted from the game's Z-up axes by the writer.
    pub basis: Basis,
    /// The unit: always `metre`.
    pub unit: Unit,
}

/// The choices a package was written with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Options {
    /// The level of detail of every model, 0 for the most detailed; a model that lacks it is
    /// written at the nearest lower level.
    pub lod: u32,
    /// The mip level of the textures, 0 for the largest; a texture with fewer mips gives its
    /// smallest.
    pub texture_mip: u32,
    /// Whether the meshes carry the vertex colours of their models.
    pub vertex_colours: bool,
    /// The ports left out of the Loadout, as the paths `entity loadout` lists.
    pub exclude: Vec<String>,
    /// The Entities attached to a port instead of what it holds.
    pub replace: Vec<Replacement>,
}

/// An Entity attached to a port instead of what the Loadout puts there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Replacement {
    /// The port's path.
    pub port: String,
    /// The Entity as the user named it.
    pub entity: String,
}

/// One node of the scene hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    /// What the node places.
    pub kind: NodeKind,
    /// The node's name: `<port>/<entity>` or the class name for the root, the name from the
    /// Interior's files for the other kinds.
    pub name: String,
    /// The path of the node from the root, the port names or, below `Interior`, the names of the
    /// nodes joined by `/`; empty for the root.
    pub path: String,
    /// The index of the parent node in `nodes`, absent for the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    /// The Entity placed at the node; present for the kind `entity`, absent for the others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<EntityRef>,
    /// The light at the node; present for the kind `light`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<Light>,
    /// The id of the mesh drawn at the node; absent when the node draws nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
    /// The translation relative to the parent, in metres.
    pub translation: [f32; 3],
    /// The rotation relative to the parent as a unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// The scale relative to the parent; a negative X marks a mirroring transform.
    pub scale: [f32; 3],
}

/// One GLB mesh: the geometry of a model drawn with one material library and one palette.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Mesh {
    /// The mesh's id, e.g. `m0001`; also the stem of its file.
    pub id: String,
    /// The GLB file relative to the package folder, e.g. `meshes/m0001.glb`.
    pub file: String,
    /// The geometry file in the Archive the model was read from, below `Data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    /// The name of the tint palette the mesh's colours were taken from, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// The id of that palette in `palettes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette_id: Option<String>,
    /// The number of nodes that draw this mesh; more than one means the mesh is instanced.
    pub instances: usize,
    /// The size of the file in bytes.
    pub bytes: u64,
    /// The number of vertices of the mesh.
    pub vertices: usize,
    /// The number of triangles of the mesh.
    pub triangles: usize,
    /// The materials the mesh is drawn with, in the order of the glTF materials in its file; empty
    /// for a mesh with a `rig`, whose materials are in the file.
    pub materials: Vec<MeshMaterial>,
    /// The skeleton, skinned parts, morph targets and animations of the file; present for a
    /// Character (version 1.3), absent for a static mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rig: Option<Rig>,
}

/// A material of a mesh and the textures its material file names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshMaterial {
    /// The index of the glTF material in the mesh's file.
    pub index: usize,
    /// The name of the glTF material in the mesh's file.
    pub name: String,
    /// The textures that were written for the material.
    pub textures: MaterialTextures,
    /// The id of the material's description in `materials`; absent for the engine's default
    /// material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
}

/// The texture ids of a material by role; a role is absent when the material names no texture for
/// it or the texture could not be written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MaterialTextures {
    /// The diffuse map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse: Option<String>,
    /// The normal map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<String>,
    /// The specular map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular: Option<String>,
}

/// One texture file of the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Texture {
    /// The texture's id, e.g. `t0001`; also the stem of its file.
    pub id: String,
    /// The image file relative to the package folder, e.g. `textures/t0001.png`.
    pub file: String,
    /// The texture's path in the Archive, below `Data`.
    pub source: String,
    /// The image format.
    pub format: TextureFormat,
    /// The bits per channel; 8 in version 1.
    pub bit_depth: u8,
    /// How the colour channels are encoded.
    pub colour_space: ColourSpace,
    /// What the alpha channel holds.
    pub alpha: AlphaMeaning,
    /// The width in pixels of the mip level that was written.
    pub width: u32,
    /// The height in pixels of the mip level that was written.
    pub height: u32,
}

/// Something that could not be resolved or written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    /// What reported it.
    pub origin: IssueOrigin,
    /// A stable code of the kind, e.g. `geometry_missing` or `texture`.
    pub kind: String,
    /// The port path of the node, or the texture file as the material file names it.
    pub path: String,
    /// The class name of the node's Entity, or the name of the material.
    pub name: String,
    /// Whether the node is left out of the scene.
    pub skipped: bool,
    /// The reason in words. Untrusted text from the Archive.
    pub reason: String,
}
