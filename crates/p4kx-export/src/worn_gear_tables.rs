// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::{ComponentKind, PhysicalComponents};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::component_sources::physical_component;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;

#[path = "catalog_clothing.rs"]
mod clothing;
#[path = "catalog_suit_armor.rs"]
mod suit_armor;

pub use clothing::{
    Clothing, Flight as ClothingFlight, RadiationResistance, TemperatureResistance,
};
pub(crate) use suit_armor::MACRO;
pub use suit_armor::{DamageResistanceMacro, MacroBody, SuitArmor};

const SUIT_ARMOR: ComponentKind = ComponentKind {
    source_type: suit_armor::COMPONENT,
    inspection: ConfigurationInspection::SuitArmor,
};
const CLOTHING: ComponentKind = ComponentKind {
    source_type: clothing::COMPONENT,
    inspection: ConfigurationInspection::Clothing,
};

physical_component!(SuitArmorComponents, SuitArmor, SUIT_ARMOR);
physical_component!(ClothingComponents, Clothing, CLOTHING);

/// Gear and device records scoped by their physical component owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WornGearTables<'source> {
    suit_armor: CatalogTable<SuitArmor<'source>>,
    clothing: CatalogTable<Clothing<'source>>,
}

impl<'source> WornGearTables<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            suit_armor: components.table(SUIT_ARMOR, SuitArmor::read)?,
            clothing: components.table(CLOTHING, Clothing::read)?,
        })
    }

    pub(super) fn resistance_indices(&self) -> impl Iterator<Item = u32> + '_ {
        self.suit_armor
            .rows
            .iter()
            .filter_map(SuitArmor::macro_index)
    }
}
