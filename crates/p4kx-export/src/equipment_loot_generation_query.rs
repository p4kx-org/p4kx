// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_independent_configurations::loot_generation_component::LootGenerationComponents;

pub use super::ComponentOwner;
pub use crate::catalog_v3::catalog_independent_configurations::loot_generation_component::LootGenerationComponent;

equipment_component_query!(
    LootGenerationComponents,
    LootGenerationComponent,
    LootGenerationSnapshot,
    with_loot_generation_components,
    with_loot_generation_components_from
);
