// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::{ComponentKind, PhysicalComponents};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::referenced_definition_read::{self, Selection};
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;

#[path = "catalog_clothing.rs"]
mod clothing;
#[path = "catalog_item_radar.rs"]
mod radar;
#[path = "catalog_suit_armor.rs"]
mod suit_armor;

use clothing::Clothing;
use radar::ItemRadar;
pub(crate) use suit_armor::MACRO;
use suit_armor::SuitArmor;
pub use suit_armor::{DamageResistanceMacro, MacroBody};

const SUIT_ARMOR: ComponentKind = ComponentKind {
    source_type: suit_armor::COMPONENT,
    inspection: ConfigurationInspection::SuitArmor,
};
const CLOTHING: ComponentKind = ComponentKind {
    source_type: clothing::COMPONENT,
    inspection: ConfigurationInspection::Clothing,
};
const ITEM_RADAR: ComponentKind = ComponentKind {
    source_type: radar::COMPONENT,
    inspection: ConfigurationInspection::ItemRadar,
};

/// Gear and device records scoped by their physical component owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WornGearTables<'source> {
    suit_armor: CatalogTable<SuitArmor<'source>>,
    clothing: CatalogTable<Clothing<'source>>,
    item_radars: CatalogTable<ItemRadar<'source>>,
}

impl<'source> WornGearTables<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            suit_armor: components.table(SUIT_ARMOR, SuitArmor::read)?,
            clothing: components.table(CLOTHING, Clothing::read)?,
            item_radars: components.table(ITEM_RADAR, ItemRadar::read)?,
        })
    }

    pub(super) fn resistance_macros(
        &self,
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<CatalogTable<DamageResistanceMacro<'source>>, CatalogError> {
        let linked = self
            .suit_armor
            .rows
            .iter()
            .filter_map(SuitArmor::macro_index)
            .collect();
        let definitions = referenced_definition_read::read(
            components.forge,
            components.projection.original(),
            &Selection {
                source_types: &[MACRO],
                linked: &linked,
                filter: components.options.filter,
                inspection: ConfigurationInspection::DamageResistanceMacro,
            },
            components.options.cancel,
            DamageResistanceMacro::read,
        )?;
        components.numbers.extend(definitions.numbers);
        components.collected.ledger.merge(definitions.ledger);
        components.collected.add_failures(definitions.failures);
        Ok(CatalogTable::new(
            components.forge,
            &[MACRO],
            definitions.rows.into_iter().map(|(_, row)| row).collect(),
        ))
    }
}
