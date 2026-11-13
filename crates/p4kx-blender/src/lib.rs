// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Blender package service of P4KX: writes an [`Assembly`](p4kx_assembly::Assembly) as a
//! decomposed package that does not depend on Blender's file format.
//!
//! A package is a folder:
//!
//! ```text
//! <package>/
//!   scene.json        the manifest: version, source, node hierarchy, meshes, textures, issues
//!   meshes/m0001.glb  one GLB per distinct mesh set (model, material library and palette)
//!   textures/t0001.png  one PNG per distinct texture of the materials
//! ```
//!
//! [`write_package`] writes it; [`write_character_package`] writes a Character, one skinned GLB
//! with its skeleton, morph targets and animations, as the same kind of folder. [`Manifest`] is
//! the type of `scene.json` and [`manifest_schema`] its JSON Schema, checked in as `schemas/blender-package.schema.json`. The file names are made
//! of ids, never of names from the Archive, so no name can leave the package folder. The format
//! is documented in `docs/blender-package.md`.

mod build;
mod character;
mod collect;
mod creature;
mod describe;
mod error;
mod finite;
mod interior;
mod items;
mod manifest;
mod manifest_light;
mod manifest_materials;
mod manifest_mesh;
mod manifest_rig;
mod material_library_sources;
mod mesh_options;
mod meshes;
#[cfg(test)]
mod owned_output_tests;
mod package_default_material;
mod package_destination;
mod package_paths;
mod package_replacement;
mod package_soc_scene;
mod package_write_context;
mod plan;
mod player_source;
mod player_source_body;
mod player_source_classes;
mod player_source_loadout;
mod player_source_motion;
mod player_source_port_binding;
mod player_source_port_helper;
mod rename;
mod rig_package;
mod roles;
mod schema;
mod soc_scene;
mod soc_scene_class_candidates;
mod soc_scene_class_components;
mod soc_scene_class_loadout;
mod soc_scene_class_mannequin;
mod soc_scene_class_resources;
mod soc_scene_class_sensor;
mod soc_scene_classes;
mod soc_scene_entities;
mod soc_scene_light_classes;
mod soc_scene_light_defaults;
mod soc_scene_light_override;
mod soc_scene_light_value;
mod soc_scene_lights;
mod soc_scene_loadouts;
mod soc_scene_occurrence;
mod soc_scene_origin;
mod soc_scene_rooms;
mod soc_scene_stream;
mod soc_scene_tile_ports;
mod soc_scene_xml;
mod stage;
mod texture_dds;
mod texture_encoding;
mod texture_field_origins;
mod texture_format;
mod texture_source_metadata;
mod textures;
mod write;

pub use character::{
    CharacterScene, write_character_package, write_character_package_with_context,
};
pub use error::PackageError;
pub use interior::{InteriorScene, write_interior_package, write_interior_package_with_context};
pub use manifest::{
    AlphaMeaning, Basis, BuildInfo, CharacterSource, ColourSpace, Coordinates, EntityRef, FORMAT,
    FORMAT_VERSION, Format, Generator, Issue, IssueOrigin, MANIFEST_FILE, MESHES_FOLDER, Manifest,
    MaterialTextures, Mesh, MeshMaterial, Node, NodeKind, Options, Replacement, Source,
    TEXTURES_FOLDER, Texture, Unit,
};
pub use manifest_light::{Light, LightKind};
pub use manifest_materials::{
    AlphaMode, Element, MaterialDescription, MaterialLayer, MaterialLibrarySourceDescription,
    MaterialStencil, MaterialTexture, Palette, PaletteColours, PaletteEntry, Parameter, TextureUse,
};
pub use manifest_rig::{Rig, RigClip, RigPart, RigPartOrigin};
pub use package_default_material::{FallbackMaterialCause, FallbackMaterialSlot};
pub use package_soc_scene::ManifestSocScene;
pub use package_write_context::PackageWriteContext;
pub use schema::manifest_schema;
pub use soc_scene::{SOC_SCENE_VERSION, SocScene, SocSceneError, SocXmlPosition, soc_scene_schema};
pub use stage::{check_output, check_output_with_context};
pub use texture_dds::{
    DdsTextureAsset, DdsTextureFallback, DdsTextureSourceChecksum, PackageTextureContainer,
};
pub use texture_encoding::{TextureChannelTransform, TextureEncoding};
pub use texture_format::TextureFormat;
pub use texture_source_metadata::TextureSourceMetadata;
pub use write::{PackageInfo, PackageSummary, write_package, write_package_with_context};

pub(crate) type PackageResult<T> = Result<T, PackageError>;

pub use player_source::{PlayerSource, PlayerSourceError, player_source_schema};
