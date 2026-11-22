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

pub use actor_procedural_recoil_config::{ActorProceduralRecoilConfig, ActorProceduralRecoilSetup};
pub(crate) use actor_procedural_recoil_config::{
    RECOIL_CONFIG_SOURCE_TYPE, RECOIL_MODIFIERS_SOURCE_TYPE,
};
pub use actor_state_filter::ActorStateFilter;
pub(crate) use weapon_procedural_animation::{ANIMATION_SOURCE_TYPE, CLIP_SOURCE_TYPE};
pub use weapon_procedural_animation::{WeaponProceduralAnimation, WeaponProceduralClipSetup};
pub use weapon_procedural_tables::WeaponProceduralTarget as ActorProceduralRecoilModifier;
pub use weapon_procedural_tables::WeaponProceduralTarget as WeaponProceduralClip;
pub(in crate::catalog_v3) use weapon_procedural_tables::read;
pub(crate) use weapon_procedural_tables::{
    WeaponProceduralTables, read_animation_selected, read_procedural_target_selected,
    read_recoil_config_selected,
};
