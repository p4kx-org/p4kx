// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::Serialize;

use super::PhysicalComponents;
use super::item_radar::{self, ItemRadar, RadarSystemSharedParams, SHARED_RADAR_PARAMS};
use super::worn_gear::{DamageResistanceMacro, MACRO, WornGearTables};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::referenced_definition_read;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemLinkedDefinitions<'source> {
    damage_resistance_macros: CatalogTable<DamageResistanceMacro<'source>>,
    radar_system_shared_params: CatalogTable<RadarSystemSharedParams<'source>>,
}

pub(super) fn read_for_items<'source>(
    components: &mut PhysicalComponents<'_, '_, 'source>,
    worn_gear: &WornGearTables<'source>,
) -> Result<
    (
        CatalogTable<ItemRadar<'source>>,
        ItemLinkedDefinitions<'source>,
    ),
    CatalogError,
> {
    let radars = components.table(item_radar::KIND, ItemRadar::read)?;
    let definitions =
        ItemLinkedDefinitions::read(components, worn_gear.resistance_indices(), &radars.rows)?;
    Ok((radars, definitions))
}

impl<'source> ItemLinkedDefinitions<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
        armor_targets: impl Iterator<Item = u32>,
        radars: &[ItemRadar<'source>],
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_resistance_macros: read(
                components,
                MACRO,
                &armor_targets.collect(),
                ConfigurationInspection::DamageResistanceMacro,
                DamageResistanceMacro::read,
            )?,
            radar_system_shared_params: read(
                components,
                SHARED_RADAR_PARAMS,
                &radars
                    .iter()
                    .filter_map(ItemRadar::shared_params_index)
                    .collect(),
                ConfigurationInspection::RadarSystemSharedParams,
                RadarSystemSharedParams::read,
            )?,
        })
    }
}

fn read<'source, Row>(
    components: &mut PhysicalComponents<'_, '_, 'source>,
    source_type: &'static str,
    linked: &BTreeSet<u32>,
    inspection: ConfigurationInspection,
    project: referenced_definition_read::Projection<'source, Row>,
) -> Result<CatalogTable<Row>, CatalogError> {
    let definitions = referenced_definition_read::read(
        components.forge,
        components.projection.original(),
        &referenced_definition_read::Selection {
            source_types: &[source_type],
            linked,
            filter: components.options.filter,
            inspection,
        },
        components.options.cancel,
        project,
    )?;
    let rows = components.keep_definitions(definitions);
    Ok(CatalogTable::new(components.forge, &[source_type], rows))
}
