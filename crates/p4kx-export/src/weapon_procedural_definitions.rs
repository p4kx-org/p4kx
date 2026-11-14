// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "actor_procedural_recoil_config.rs"]
mod actor_procedural_recoil_config;
#[path = "actor_state_filter.rs"]
mod actor_state_filter;
#[path = "weapon_procedural_animation.rs"]
mod weapon_procedural_animation;
#[path = "weapon_procedural_source.rs"]
mod weapon_procedural_source;
#[path = "weapon_procedural_tables.rs"]
mod weapon_procedural_tables;

pub(in crate::catalog_v3) use actor_procedural_recoil_config::{
    ActorProceduralRecoilConfig, RECOIL_CONFIG_SOURCE_TYPE, RECOIL_MODIFIERS_SOURCE_TYPE,
};
pub(in crate::catalog_v3) use weapon_procedural_animation::{
    ANIMATION_SOURCE_TYPE, CLIP_SOURCE_TYPE, WeaponProceduralAnimation,
};
pub(crate) use weapon_procedural_tables::WeaponProceduralTables;
pub(in crate::catalog_v3) use weapon_procedural_tables::read;
