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
mod animation_clip_coordinates;
mod animation_controller;
mod animation_dba;
mod animation_error;
mod animation_header;
mod animation_keys;
mod animation_read;
mod animation_source;
mod bone;
mod bone_extend;
mod bone_map;
mod bone_match;
mod build;
mod build_lod;
mod caf_image;
pub mod character_definition;
mod companion;
mod controller_binding;
mod crch;
mod crch_mesh;
mod crch_node;
mod dba_animation_sources;
mod dba_metadata_records;
mod error;
mod hair_cap;
mod hierarchy;
mod included;
mod lod_skeleton_origin;
mod material;
mod material_companion;
mod material_emission;
mod material_read_observation;
mod material_request;
mod mesh;
mod mesh_chunk_source;
mod mesh_origin;
mod mesh_stream_source;
mod model;
mod model_bone_sources;
mod model_chunk_layout;
mod model_chunk_words;
mod model_node_coordinates;
mod model_node_selection;
mod model_node_sources;
mod model_options;
mod mtl;
mod mtl_direct_read;
mod mtl_error;
mod mtl_layer_cache;
mod mtl_layer_shares;
mod mtl_layers;
mod mtl_library;
mod mtl_load;
mod mtl_material_inputs;
mod mtl_material_read;
mod mtl_number;
mod mtl_observe_ignore;
mod mtl_palette_read;
mod mtl_parse;
mod mtl_public_value;
mod mtl_raw;
mod mtl_read_site;
mod mtl_relief;
mod mtl_render_to_texture;
mod mtl_role_read;
mod mtl_roles;
mod mtl_shader;
mod mtl_size;
mod mtl_source;
mod mtl_source_capture;
mod mtl_source_error;
mod mtl_source_read;
mod mtl_source_size;
mod mtl_stencil;
mod mtl_stencil_read;
mod mtl_texture;
mod mtl_unresolved;
mod mtl_vertex_colours;
mod mtl_xml;
mod mtl_xml_source;
mod nmc;
mod nmc_frames;
mod nmc_properties;
mod nmc_transforms;
mod object_container;
mod palette;
mod physics_blob;
mod physics_trimesh;
mod positioned_animation;
mod prefab;
mod reader;
mod rig_logic_bytes;
mod second_uv;
mod skeleton;
mod skeleton_source;
mod skin;
mod smooth_normals;
mod soc_area_component;
mod soc_container_descriptor;
mod soc_entities;
mod soc_entity_component;
mod soc_entity_links;
mod soc_entity_source;
mod soc_geometry;
mod soc_group_placement;
mod soc_inline_loadout;
mod soc_light;
mod soc_light_projection;
mod soc_light_source;
mod soc_room_registry;
mod soc_room_segment;
mod soc_source_node;
mod soc_tile_port;
mod stat_obj_physics;
mod streams;
mod transform;
mod vector;

pub use animation::{Animation, BoundTracks, Keys, Track};
pub use animation_clip_coordinates::{
    AnimationChunkCoordinates, AnimationClipCoordinates, AnimationDatabaseSelection,
    AnimationFrameChannel, AnimationLastFrame,
};
pub use animation_controller::{AnimationChannel, AnimationController, DecodedAnimationChannel};
pub use animation_dba::{Dba, DbaHeaders, SkippedAnimation};
pub use animation_error::{AnimationError, CorruptAnimation};
pub use animation_header::AnimationHeader;
pub use animation_read::{MAX_ANIMATION_KEYS, MAX_DATABASE_KEYS};
pub use animation_source::{AnimationMetadata, AnimationSource, AnimationTiming, DbaAnimationName};
pub use bone::{Bone, BoneTransform};
pub use bone_extend::{
    chained_world_poses, chained_world_poses_observing, extend_skeleton, extend_skeleton_placed,
    multiply as multiply_quaternions,
};
pub use bone_map::{Influences, MAX_INFLUENCES};
pub use bone_match::BoneMatch;
pub use caf_image::{CafImage, CafImageError, CafImagePath, CafImageRecord};
pub use companion::{
    CompanionError, CompanionPath, CompanionResolver, GREY_PROBE_MIP, MAX_GREY_SATURATION,
    companion_file_names,
};
pub use controller_binding::{ControllerBinder, ControllerBinding, ControllerEligibility};
pub use dba_animation_sources::DbaAnimationSources;
pub use dba_metadata_records::{DbaMetadataRecord, DbaMetadataRecords};
pub use error::{CorruptChunk, ModelError, UnsupportedLayout};
pub use included::{
    IncludedObject, IncludedObjects, IncludedObjectsError, MAX_INCLUDED_MODELS,
    MAX_INCLUDED_OBJECTS,
};
pub use lod_skeleton_origin::LodSkeletonOrigin;
pub use material_companion::{MaterialCompanion, MaterialCompanionUse, MaterialReadOrigin};
pub use material_emission::{
    EmissionAttribute, EmissionAttributeKind, EmissionNumber, EmissionNumeric, EmissionTextureSlot,
    EmissionValue, MaterialEmission,
};
pub use material_read_observation::{MaterialReadField, MaterialReadObservation, MaterialReadUse};
pub use material_request::MaterialRequest;
pub use mesh_chunk_source::{MeshChunkSource, MeshCounts, MeshSubmeshSource, MeshVertexReference};
pub use mesh_origin::MeshOrigin;
pub use mesh_stream_source::{MeshStreamHeader, MeshStreamOccurrence, MeshStreamSource};
pub use model::{ChunkUsage, Lod, LodSelection, Mesh, Model, ModelChunk, ModelFile, Node, Submesh};
pub use model_bone_sources::{ModelBoneSource, ModelBoneSources, PrimarySkeletonSelection};
pub use model_chunk_layout::ModelChunkLayout;
pub use model_chunk_words::ModelChunkWords;
pub use model_chunk_words::ModelChunkWords as SkeletonTypePrefix;
pub use model_node_coordinates::{ModelNodePropertySource, ModelNodeSource};
pub use model_node_selection::{ModelNodeMeshChunk, ModelNodeSelection};
pub use model_node_sources::{ModelNodeSources, ModelReadSources};
pub use model_options::{ModelIssue, PositionBox};
pub use mtl::{
    MAX_SUB_MATERIALS, Material, MaterialFlags, MaterialLibrary, MaterialStatus, TextureSlot,
};
pub use mtl_error::MaterialError;
pub use mtl_layer_cache::{
    LayerBudget, LayerCache, LayerLimits, MAX_CACHED_LAYER_FILES, MAX_LAYER_BYTES, MAX_LAYER_FILES,
};
pub use mtl_layers::LayerSurface;
pub use mtl_load::{MaterialReading, request_path as material_library_path};
pub use mtl_raw::{Attributes, MAX_RAW_DEPTH, MAX_RAW_ELEMENTS, MaterialLayer, RawElement};
pub use mtl_roles::{TextureRole, is_diffuse_map};
pub use mtl_source::{
    AuthoredAttribute, AuthoredAttributeKind, AuthoredContent, ElementAuthored, MaterialEncoding,
    MaterialEntryDisposition, MaterialSource, MaterialSourceEntry, MaterialSourceLimits,
    MaterialSourceLoss, MaterialSourceLossKind,
};
pub use mtl_source_error::MaterialReadError;
pub use mtl_stencil::{STENCIL_SLOT, Stencil, StencilTint};
pub use mtl_texture::{DIFFUSE_SLOT, texture_suffix, texture_suffix_lowercase};
pub use mtl_unresolved::UnresolvedLayers;
pub use nmc_frames::{NmcFrame, NmcFrameField, NmcFrames};
pub use nmc_transforms::{NmcFrameProblem, NmcNodeTransforms, NmcTransformError, NmcTransforms};
pub use object_container::{
    ChildContainer, ContainerBounds, MAX_CHILD_CONTAINERS, MAX_STREAMED_SOCS, MAX_STREAMING_DEPTH,
    ObjectContainer, ObjectContainerError, is_external_child_value,
};
pub use palette::{PaletteSlot, TintEntry, TintPalette, srgb8_to_linear};
pub use physics_blob::{
    PhysicsAxial, PhysicsBlob, PhysicsBlobError, PhysicsBox, PhysicsFloat, PhysicsGeometry,
    PhysicsSphere, physics_marker_range, physics_shape_range,
};
pub use physics_trimesh::PhysicsTrimesh;
pub use positioned_animation::{PositionedAnimation, PositionedDba};
pub use prefab::{
    MAX_GROUP_DEPTH, MAX_LIBRARY_OBJECTS, MAX_LIBRARY_PREFABS, Prefab, PrefabError, PrefabLibrary,
    PrefabObject, PrefabRef,
};
pub use rig_logic_bytes::RigLogicBytes;
pub use skeleton_source::{PositionedSkeleton, SkeletonSource};
pub use soc_area_component::{SocAreaComponent, SocAreaLink};
pub use soc_container_descriptor::{
    SocContainerDeclaration, SocContainerDescriptor, SocContainerDescriptorError,
    SocStreamDeclaration,
};
pub use soc_entities::{
    MAX_SOC_ENTITIES, MAX_SOC_PLACED, SocClassEntity, SocEntities, SocEntitiesError, SocPalette,
    SocProp,
};
pub use soc_entity_component::{SocEntityComponent, SocEntityComponentKind};
pub use soc_entity_links::{SocEntityLink, SocEntityLinks};
pub use soc_entity_source::{SocEntity, SocEntityChunk, SocEntityFile, SocEntitySourceError};
pub use soc_group_placement::{SocGroupCoordinates, SocGroupPlacement};
pub use soc_inline_loadout::{
    SocInlineLoadout, SocInlineLoadoutEntries, SocInlineLoadoutEntry, SocInlineLoadoutKind,
};
pub use soc_light::{LightState, SocLight};
pub use soc_light_source::{
    SocBakedLightSource, SocLightGroupSource, SocLightMode, SocLightSource,
};
pub use soc_room_registry::{SocRoomDeclaration, SocRoomRegistry, SocRoomRegistryError};
pub use soc_room_segment::{
    SocRoomSegment, SocSegmentGeometry, SocSegmentPoint, SocSegmentTransform,
};
pub use soc_source_node::SocSourceValue;
pub use soc_tile_port::{SocTileAttachment, SocTileItemPort, tile_attachment_point_suffix};
pub use stat_obj_physics::{StatObjPhysics, StatObjPhysicsError, StatObjPhysicsSlot};
pub use transform::Transform;
