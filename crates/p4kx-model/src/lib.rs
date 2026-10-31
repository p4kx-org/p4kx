// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Interprets `CryEngine` chunk files as models.
//!
//! [`Model::read`] builds a [`Model`] from a parsed [`p4kx_chunks::ChunkFile`] and the companion
//! files a [`CompanionResolver`] supplies: its [`Node`]s with their transforms and hierarchy, the
//! [`Mesh`]es of each [`Lod`] with positions, normals, tangents, texture coordinates, vertex
//! colours, bone weights ([`Influences`]) and indices, the [`Bone`]s of the skeleton with their
//! bind poses, and the
//! [`Submesh`]es that draw them with one material ID each, and the [`MaterialLibrary`] of the
//! `.mtl` file the IDs index. Coordinates stay in the `CryEngine`
//! convention, Z up, in metres. Chunks the model does not read are listed with their
//! [`ChunkUsage`].
//!
//! [`Animation::read_caf`] and [`Dba::read`] read the animations of `.caf` files and `.dba`
//! animation databases into [`Track`]s of rotation and translation [`Keys`] per controller;
//! [`Animation::bind`] matches them to the [`Bone`]s of a skeleton by controller ID.
//!
//! [`ObjectContainer::parse`] reads the description of a socpak's object container, the
//! containers nested in it and the `.soc` files it streams, and [`IncludedObjects::read`] the
//! models a `.soc` file places, each with its [`Transform`]; [`SocEntities::read`] reads its
//! props, which name geometry of their own, the entities that name a class Record, and its
//! [`SocLight`]s.
//!
//! A mesh that stores no normals gets smooth ones computed from its triangles
//! ([`Mesh::normals_computed`]). Quantised positions are dequantised with the scaling box of the
//! mesh chunk, or with the model box on request ([`PositionBox`], [`Model::read_with_box`]).
//!
//! Both containers are read: an `#ivo` model keeps its meshes in a geometry companion, a `CrCh`
//! model in its own file, as the physics mesh a `Mesh` chunk refers to (docs/crch-models.md).

mod animation;
mod animation_dba;
mod animation_error;
mod animation_header;
mod animation_keys;
mod animation_read;
mod bone;
mod bone_extend;
mod bone_map;
mod bone_match;
mod build;
mod build_lod;
mod companion;
mod crch;
mod crch_mesh;
mod crch_node;
mod error;
mod hair_cap;
mod hierarchy;
mod included;
mod material;
mod mesh;
mod model;
mod model_options;
mod mtl;
mod mtl_error;
mod mtl_layer_cache;
mod mtl_layer_shares;
mod mtl_layers;
mod mtl_load;
mod mtl_parse;
mod mtl_raw;
mod mtl_relief;
mod mtl_render_to_texture;
mod mtl_roles;
mod mtl_shader;
mod mtl_size;
mod mtl_stencil;
mod mtl_texture;
mod mtl_unresolved;
mod mtl_vertex_colours;
mod mtl_xml;
mod nmc;
mod object_container;
mod palette;
mod prefab;
mod reader;
mod second_uv;
mod skeleton;
mod skin;
mod smooth_normals;
mod soc_entities;
mod soc_geometry;
mod soc_light;
mod streams;
mod transform;
mod vector;

pub use animation::{Animation, BoundTracks, Keys, Track};
pub use animation_dba::{Dba, DbaHeaders, SkippedAnimation};
pub use animation_error::{AnimationError, CorruptAnimation};
pub use animation_header::AnimationHeader;
pub use animation_read::{MAX_ANIMATION_KEYS, MAX_DATABASE_KEYS};
pub use bone::{Bone, BoneTransform};
pub use bone_extend::{
    chained_world_poses, extend_skeleton, extend_skeleton_placed, multiply as multiply_quaternions,
};
pub use bone_map::{Influences, MAX_INFLUENCES};
pub use bone_match::BoneMatch;
pub use companion::{
    CompanionError, CompanionPath, CompanionResolver, GREY_PROBE_MIP, MAX_GREY_SATURATION,
    companion_file_names,
};
pub use error::{CorruptChunk, ModelError, UnsupportedLayout};
pub use included::{
    IncludedObject, IncludedObjects, IncludedObjectsError, MAX_INCLUDED_MODELS,
    MAX_INCLUDED_OBJECTS,
};
pub use model::{ChunkUsage, Lod, LodSelection, Mesh, Model, ModelChunk, ModelFile, Node, Submesh};
pub use model_options::{ModelIssue, PositionBox};
pub use mtl::{
    MAX_SUB_MATERIALS, MAX_TEXTURE_SLOTS, Material, MaterialFlags, MaterialLibrary, MaterialStatus,
    TextureSlot,
};
pub use mtl_error::MaterialError;
pub use mtl_layer_cache::{
    LayerBudget, LayerCache, LayerLimits, MAX_CACHED_LAYER_FILES, MAX_LAYER_BYTES, MAX_LAYER_FILES,
};
pub use mtl_layers::LayerSurface;
pub use mtl_load::MaterialReading;
pub use mtl_raw::{
    Attributes, MAX_LAYERS, MAX_RAW_DEPTH, MAX_RAW_ELEMENTS, MaterialLayer, RawElement,
};
pub use mtl_roles::{TextureRole, is_diffuse_map};
pub use mtl_stencil::{STENCIL_SLOT, Stencil};
pub use mtl_texture::{DIFFUSE_SLOT, texture_suffix, texture_suffix_lowercase};
pub use mtl_unresolved::UnresolvedLayers;
pub use object_container::{
    ChildContainer, ContainerBounds, MAX_CHILD_CONTAINERS, MAX_STREAMED_SOCS, MAX_STREAMING_DEPTH,
    ObjectContainer, ObjectContainerError,
};
pub use palette::{PaletteSlot, TintEntry, TintPalette, srgb8_to_linear};
pub use prefab::{
    MAX_GROUP_DEPTH, MAX_LIBRARY_OBJECTS, MAX_LIBRARY_PREFABS, Prefab, PrefabError, PrefabLibrary,
    PrefabObject, PrefabRef,
};
pub use soc_entities::{
    MAX_SOC_ENTITIES, MAX_SOC_PLACED, SocClassEntity, SocEntities, SocEntitiesError, SocPalette,
    SocProp,
};
pub use soc_light::{LightState, SocLight};
pub use transform::Transform;
