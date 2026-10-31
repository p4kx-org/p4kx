// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writers for synthetic test data in the formats P4KX reads.
//!
//! The writers reproduce the byte layout of real Star Citizen files so that every reader can be
//! tested without game files; [`real_archive_path`] locates a real Archive for the opt-in suite.
//! This crate is a development dependency only.

mod adpcm;
mod animation;
mod animation_dba;
mod animation_keys;
mod archive_builder;
mod audio;
mod bnk;
mod build_manifest;
mod bytes;
mod chf;
mod chunk_file;
mod cli;
mod container;
mod cryxml;
mod dataforge_builder;
mod dataforge_instances;
mod dataforge_schema;
mod dataforge_strings;
mod dataforge_value;
mod dataforge_wide;
mod dna;
mod entity;
mod entity_armour;
mod entity_armour_loadout;
mod entity_assembly;
mod entity_assortment;
mod entity_bone;
mod entity_cargo;
mod entity_clothing;
mod entity_creature;
mod entity_creature_animated;
mod entity_dressed;
mod entity_fit_sample;
mod entity_gear;
mod entity_head;
mod entity_interior_sample;
mod entity_layer_chain;
mod entity_layered;
mod entity_livery;
mod entity_loadout;
mod entity_module_sample;
mod entity_mount;
mod entity_npc;
mod entity_npc_face;
mod entity_npc_generic;
mod entity_npc_zones;
mod entity_painted;
mod entity_port;
mod entity_prop_rig;
mod entity_requirement;
mod entity_sample;
mod entity_schema;
mod entity_skin;
mod entity_vehicle;
mod entity_zones;
mod frames;
mod gfx;
mod gfx_import;
mod gfx_place;
mod gfx_shape;
mod gfx_tags;
mod gfx_text;
mod glb;
mod glb_animation;
mod glb_container;
mod glb_images;
mod glb_lights;
mod glb_morph;
mod glb_skin;
mod global_ini;
mod hirc_music;
mod hirc_node;
mod interior_classes;
mod interior_lit;
mod interior_sample;
mod layout_v1;
mod layout_v2;
mod mesh_streams;
mod model;
mod model_crch;
mod model_cube;
mod model_layout;
mod model_nmc;
mod model_rig;
mod model_skeleton;
mod model_skin;
mod model_zoned;
mod mtl;
mod opus;
mod opus_encode;
mod prefab;
mod real_archive;
mod reference_output;
mod rig_logic;
mod screens;
mod screens_animation;
mod screens_canvas;
mod screens_geometry;
mod screens_image;
mod screens_image_schema;
mod screens_radar_schema;
mod screens_rest;
mod screens_sample;
mod screens_style;
mod screens_text;
mod screens_text_schema;
mod screens_value;
mod screens_widget;
mod skin_blendshapes;
mod soc;
mod soc_entities;
mod split_dds;
mod test_entry;
mod test_jpeg;
mod test_key;
mod timing;
mod vorbis;
pub mod watchdog;
mod wem;

pub use animation::{TestAnimation, TestTrack};
pub use animation_dba::TestDba;
pub use animation_keys::{TestPositionFormat, TestRotationFormat, TestTimes};
pub use archive_builder::ArchiveBuilder;
pub use audio::{TestAtl, audio_schema, wwise_short_id};
pub use bnk::{TestBank, TestHircObject};
pub use build_manifest::write_build_manifest;
pub use chf::{TestChf, TestItemPort, TestMaterial, TestSubMaterial};
pub use chunk_file::TestChunkFile;
pub use cli::p4kx;
pub use container::TestContainer;
pub use cryxml::{CryXmlByteOrder, CryXmlSignature, TestElement};
pub use dataforge_builder::DataForgeBuilder;
pub use dataforge_schema::{TestPool, TestRecord, TestStruct, TestType};
pub use dataforge_value::{TestInstance, TestValue};
pub use dataforge_wide::many_paths_database;
pub use dna::{TestCell, TestControl, TestDeltaRecord, TestDna};
pub use entity::TestEntity;
pub use entity_armour::{armour_entries, armour_female_skeleton, armour_male_skeleton};
pub use entity_armour_loadout::{ARMOUR_XML_LOADOUT, armour_xml_loadout};
pub use entity_assembly::{ASSEMBLY_GUN_PAINT, assembly_entries, hologram_entries};
pub use entity_assortment::{TestGroup, assortment_record, assortment_record_requiring};
pub use entity_bone::{
    BONE_THRUSTER, bone_thruster_definition, bone_thruster_entries, bone_thruster_housing,
};
pub use entity_cargo::{TestCapacity, closed_container_record, open_container_record};
pub use entity_clothing::{TestClothingCondition, TestZoneChunk, clothing_component};
pub use entity_creature::{
    CREATURE, CREATURE_ANIMATED, CREATURE_ANIMATION_DATABASE_LOST, CREATURE_ANIMATION_NESTED_LOST,
    CREATURE_ANIMATION_PARAMS_LOST, CREATURE_ASSORTED, CREATURE_COLLAR_LOST, CREATURE_COLLAR_NODED,
    CREATURE_COLLARED, CREATURE_DEFINITION, CREATURE_HORNED, CREATURE_LOADOUT_LOST,
    CREATURE_NO_SKELETON, CREATURE_SHORT_SKELETON, RIGGED_PROP, creature_entries,
};
pub use entity_dressed::{
    DRESSED_BROWS, DRESSED_EYES, DRESSED_EYES_MATERIAL, DRESSED_GONE_HAIR, DRESSED_HAIR,
    DRESSED_HAIR_MATERIAL, DRESSED_HAIR_OPACITY, DRESSED_HEAD_DIFFUSE, DRESSED_HEAD_MATERIAL,
    DRESSED_HEAD_MATERIAL_FILE, DRESSED_JAW_BONE, DRESSED_JAW_INDEX, DRESSED_UNKNOWN_MATERIAL,
    dressed_entries, dressed_jaw_entries,
};
pub use entity_fit_sample::{
    ENTITY_FIT_CLASS, ENTITY_FIT_MERGED_CLASS, ENTITY_FIT_MOVED_CLASS, entity_fit_entries,
};
pub use entity_gear::{LANDING_GEAR_SHIP, landing_gear_entries};
pub use entity_head::{
    FEMALE_BODY_TYPE, FEMALE_RIG_HASH, HEAD_REGIONS, HEAD_VERTICES, MALE_BODY_TYPE,
    MALE_HEAD_FOLDER, MALE_RIG, MALE_RIG_HASH, head_entries, head_rig,
};
pub use entity_interior_sample::{
    ENTITY_INTERIOR_CLASS, ENTITY_INTERIOR_FILE, ENTITY_LIT_ROOM_CLASS, entity_interior_entries,
    entity_lit_room_entries,
};
pub use entity_layer_chain::{LAYER_CHAIN_LENGTH, layer_chain_entries};
pub use entity_layered::{LAYERED_MISSING_TEXTURE, LAYERED_TINTS, layered_entries};
pub use entity_livery::{
    LIVERY_DEFAULT_PALETTE, LIVERY_DEFAULT_TINTS, LIVERY_GUN_TINTS, LIVERY_SHIP_TINTS,
    livery_entries,
};
pub use entity_loadout::{TestLoadout, TestLoadoutEntry};
pub use entity_module_sample::{ENTITY_MODULE_CARRIER, ENTITY_MODULE_CLASS, entity_module_entries};
pub use entity_mount::{
    MOUNT_BONE, MOUNT_COURIER, MOUNT_DRIFTER, SATCHEL_HELPER_Z, mounted_item_entries,
};
pub use entity_npc::{
    NPC_BROW_SKIN, NPC_CLASS, NPC_EYES_MATERIAL, NPC_FILES_CLASS, NPC_HAIR_BLONDE, NPC_HAIR_BROWN,
    NPC_HEAD_CLASS, NPC_HEAD_FLAGS, NPC_HEAD_FOLDER, NPC_HEAD_MATERIAL, npc_entries,
};
pub use entity_npc_face::{
    NPC_FACIAL_BONE, NPC_RIG_LOGIC, NPC_SKELETON_FILE, NPC_SKELETON_SHIFT, npc_face_entries,
    npc_face_entries_posed_by, npc_face_entries_with, npc_face_logic, npc_face_shapes,
    npc_face_skeleton,
};
pub use entity_npc_generic::{
    NPC_GENERIC_BODY_CLASS, NPC_GENERIC_CLASS, NPC_GENERIC_HEAD_CLASS, npc_generic_entries,
};
pub use entity_npc_zones::{
    NPC_ZONE_CLASS, NpcExtra, NpcZoneOptions, npc_jacketed_entries, npc_zone_entries,
};
pub use entity_painted::{
    PAINTED_FEMALE_PALETTE_GUID, PAINTED_LIBRARY_PALETTE, PAINTED_PALETTE_GUID, PAINTED_SUIT,
    painted_entries,
};
pub use entity_port::TestPort;
pub use entity_prop_rig::{
    EMPTY_ON_SKIN_PORT, GARMENT_ON_SKIN_PORT, NODE_ON_SKIN_PORT, PROP_ON_BONE_PORT,
    PROP_ON_LOST_PORT, PROP_ON_SKIN_PORT, PROP_RIG_BAD_SKELETON, PROP_RIG_CREATURE, PROP_RIG_HUMAN,
    PROP_RIG_NO_SKELETON, PROP_RIG_STATIC, SKINNED_ON_SKIN_PORT, prop_rig_entries,
};
pub use entity_requirement::TestRequirement;
pub use entity_sample::{SAMPLE_GUN, SAMPLE_MISSING, SAMPLE_MOUNT, SAMPLE_SHIP, sample_ship};
pub use entity_schema::{entity_schema, tint_palette_record};
pub use entity_skin::{SKIN_BODY, skin_attachment_entries};
pub use entity_vehicle::landing_system_record;
pub use entity_zones::{
    ZONE_JACKET, ZONE_SHIRT, jacketed_entries, zone_entries, zone_entries_with_layer_textures,
};
pub use frames::assert_steady_frames;
pub use gfx::{TestMovie, TestSignature, tag_bytes, zlib};
pub use gfx_import::{
    import_assets_body, remove_object_body, scaling_grid_body, symbol_class_body,
};
pub use gfx_place::{TestPlace, background_body, remove_object2_body};
pub use gfx_shape::{TestFill, TestGradient, TestShape, TestShapeRecord, morph_shape_body};
pub use gfx_tags::{
    TestGlyph, color_mapped_body, exporter_info_body, exports_body, external_image_body,
    font_name_body, font3_body, font3_body_styled, jpeg_body, lossless_body, place_object2_body,
    sprite_body, sub_image_body,
};
pub use gfx_text::{TestEditText, TestTextRecord, define_text_body};
pub use glb::{CheckedGlb, check_glb};
pub use glb_images::DecodedImage;
pub use global_ini::GlobalIniBuilder;
pub use gltf;
pub use hirc_music::TestPlaylistItem;
pub use hirc_node::{TestContainerKind, TestNodeBase};
pub use interior_classes::{
    CLASS_CABINET_GUID, CLASS_DEFAULT_TINTS, CLASS_DOOR_GUID, CLASS_DOOR_TINTS, CLASS_OWN_TINTS,
    CLASS_ROOM, CLASS_ROOM_TINTS, ENTITY_CLASS_ROOM_CLASS, class_room_entries,
    entity_class_room_entries,
};
pub use interior_lit::{LIT_ROOM, lit_room_entries};
pub use interior_sample::{
    INTERIOR_LAMP, INTERIOR_ROOT, INTERIOR_ROOT_NAME, interior_entries, lit_interior_entries,
};
pub use mesh_streams::{MeshStream, MeshStreams, mesh_streams};
pub use model::{
    CUBE_CAPS_MATERIAL, CUBE_MATERIAL_LIBRARY, CUBE_MESH_OFFSET, CUBE_SIDES_MATERIAL, TestModel,
    TestNode, TestNormals, TestPositions, TestSubmesh, TestVertex,
};
pub use model_crch::{CRCH_CUBE_ROOT_OFFSET, TestCrchModel};
pub use model_skeleton::{TestBone, TestSkeleton};
pub use mtl::{TestMtl, TestMtlMaterial};
pub use opus::{TestOpus, TestOpusError, TestOpusFraming, TestOpusPackets, wwise_opus_wem};
pub use prefab::{TestPrefabLibrary, TestPrefabObject, prefab_guid};
pub use real_archive::{
    MissingRealArchive, MissingRealKey, REAL_ARCHIVE_B_VAR, REAL_ARCHIVE_KEY_FILE_VAR,
    REAL_ARCHIVE_KEY_VAR, REAL_ARCHIVE_VAR, real_archive_b_path, real_archive_key,
    real_archive_pair, real_archive_path,
};
pub use reference_output::{
    REFERENCE_SCMDB_VAR, REFERENCE_SHIP_MATRIX_VAR, REFERENCE_STARBREAKER_VAR,
    REFERENCE_UNFORGE_VAR, REFERENCE_UNP4K_VAR, reference_scmdb_path, reference_ship_matrix_path,
    reference_starbreaker_path, reference_unforge_path, reference_unp4k_path,
};
pub use rig_logic::{RIG_LOGIC_CHUNK, TestLogicDeltas, TestLogicRecord, TestRigLogic};
pub use screens::{SCREEN_COLOUR_SLOTS, screens_schema};
pub use screens_animation::TestAlphaAnimation;
pub use screens_canvas::{
    TestCanvas, movie_component, named_tag_record, screen_component, tag_record,
};
pub use screens_geometry::clone_modifiers;
pub use screens_image::{
    SCREEN_IMAGE_CANVAS, screens_image_sample, screens_texture, screens_vector,
};
pub use screens_rest::{TestOp, TestParam};
pub use screens_sample::{SCREEN_MFD, SCREEN_PANEL, SCREEN_SHIP, SCREEN_STYLE, screens_sample};
pub use screens_style::{TestCondition, TestModifier, TestStyle, TestStyleEntry};
pub use screens_text::{
    SCREEN_TEXT_CANVAS, SCREEN_TEXT_STYLE, screens_font_movie, screens_text_sample,
    text_localisation,
};
pub use screens_widget::{TestColour, TestDim, TestWidget};
pub use skin_blendshapes::{BSHAPES_GPU_CHUNK, TestSkinBlendshapes, TestSkinEntry};
pub use soc::{SOC_IDENTITY, TestObjectContainer, TestSoc};
pub use soc_entities::{TestLight, TestLightState, TestSocEntity};
pub use split_dds::{SplitDds, TestTextureFormat};
pub use test_entry::TestEntry;
pub use test_jpeg::{gray_jpeg, progressive_gray_jpeg, sampled_jpeg};
pub use test_key::{other_test_key, test_key};
pub use timing::{BENCH_VAR, Timing, bench, bench_enabled, bench_once};
pub use vorbis::{TestCodebook, TestVorbis};
pub use wem::{TestCodec, TestWem, test_signal};
