// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_independent_configurations::item_radar::ItemRadarComponents;
use crate::catalog_v3::catalog_independent_configurations::worn_gear::{
    ClothingComponents, SuitArmorComponents,
};

pub use crate::catalog_v3::catalog_independent_configurations::item_radar::{
    AimAssist as ItemRadarAimAssist, ItemRadar, PingProperties as ItemRadarPingProperties,
};
pub use crate::catalog_v3::catalog_independent_configurations::worn_gear::{
    Clothing, ClothingFlight, RadiationResistance, SuitArmor, TemperatureResistance,
};

equipment_component_query!(
    SuitArmorComponents,
    SuitArmor,
    SuitArmorSnapshot,
    with_suit_armor,
    with_suit_armor_from
);
equipment_component_query!(
    ClothingComponents,
    Clothing,
    ClothingSnapshot,
    with_clothing,
    with_clothing_from
);
equipment_component_query!(
    ItemRadarComponents,
    ItemRadar,
    ItemRadarSnapshot,
    with_item_radars,
    with_item_radars_from
);
