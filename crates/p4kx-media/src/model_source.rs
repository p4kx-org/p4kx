// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "model_source/nmc.rs"]
mod nmc;
#[path = "model_source/nmc_record.rs"]
mod nmc_record;
#[path = "model_source/origins.rs"]
mod origins;
#[path = "model_source/payload.rs"]
mod payload;
#[path = "model_source/provider.rs"]
mod provider;
#[path = "model_source/schema.rs"]
mod schema;

pub(crate) use provider::ModelSourceProvider;
pub(crate) use schema::bind_record_schema;

#[path = "model_source/physics.rs"]
mod physics;

#[path = "model_source/companions.rs"]
mod companions;

#[path = "model_source/lod_distances.rs"]
mod lod_distances;
#[path = "model_source/material_library_reference.rs"]
mod material_library_reference;
#[path = "model_source/material_path.rs"]
mod material_path;

#[path = "model_source/character.rs"]
mod character;
#[path = "model_source/character_origin.rs"]
mod character_origin;
#[path = "model_source/character_payload.rs"]
mod character_payload;
#[path = "model_source/character_reference.rs"]
mod character_reference;

#[path = "model_source/skeleton.rs"]
mod skeleton;
#[path = "model_source/skeleton_record.rs"]
mod skeleton_record;

#[path = "model_source/path_reference.rs"]
mod path_reference;
#[path = "model_source/vehicle_grid.rs"]
mod vehicle_grid;
#[path = "model_source/vehicle_grid_origins.rs"]
mod vehicle_grid_origins;
#[path = "model_source/vehicle_grid_record.rs"]
mod vehicle_grid_record;

#[path = "model_source/mesh.rs"]
mod mesh;
#[path = "model_source/mesh_record.rs"]
mod mesh_record;
