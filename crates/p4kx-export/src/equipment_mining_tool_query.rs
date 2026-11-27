// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_independent_configurations::mining_configurations::{
    AttachableModifierComponents, MiningLaserComponents,
};

pub use crate::catalog_v3::catalog_independent_configurations::mining_configurations::{
    AttachableEffect, AttachableModifier, FilterEffect, FloatModifier, MiningEffect, MiningFilter,
    MiningLaser, MiningModifiers, ModifierLifetime, SalvageModifier, TractorModifier, WeaponEffect,
    WeaponModifier, WeaponModifierStats,
};

equipment_component_query!(
    MiningLaserComponents,
    MiningLaser,
    MiningLaserSnapshot,
    with_mining_lasers,
    with_mining_lasers_from
);
equipment_component_query!(
    AttachableModifierComponents,
    AttachableModifier,
    AttachableModifierSnapshot,
    with_attachable_modifiers,
    with_attachable_modifiers_from
);
