// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use super::record_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_durability::DurabilityComponent;
use crate::catalog_v3::catalog_independent_configurations::turret_health::TurretHealthComponents;
use crate::catalog_v3::catalog_item_components::ItemEquipmentFamily;

pub use crate::catalog_v3::catalog_durability::{Degradation, Distortion, Durability};
pub use crate::catalog_v3::catalog_independent_configurations::turret_health::TurretHealth;

record_query::item_component!(
    HealthRows, 'source, Durability<'source>, ItemEquipmentFamily::Durability(DurabilityComponent::Health), read_health
);
record_query::item_component!(
    DegradationRows, 'source, Degradation, ItemEquipmentFamily::Durability(DurabilityComponent::Degradation), read_degradation
);
record_query::item_component!(
    DistortionRows, 'source, Distortion, ItemEquipmentFamily::Durability(DurabilityComponent::Distortion), read_distortion
);

record_query::unlocalized!(
    HealthRows,
    Durability,
    /// Physical Health components and their original source evidence.
    DurabilitySnapshot,
    with_durability,
    with_durability_from
);

record_query::unlocalized!(
    owned DegradationRows,
    Degradation,
    /// Physical degradation components and their original source evidence.
    DegradationSnapshot,
    with_degradation,
    with_degradation_from
);

record_query::unlocalized!(
    owned DistortionRows,
    Distortion,
    /// Physical distortion components and their original source evidence.
    DistortionSnapshot,
    with_distortion,
    with_distortion_from
);

equipment_component_query!(
    TurretHealthComponents,
    TurretHealth,
    TurretHealthSnapshot,
    with_turret_health,
    with_turret_health_from
);
