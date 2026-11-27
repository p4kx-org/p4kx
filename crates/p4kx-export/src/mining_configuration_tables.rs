// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::{ComponentKind, PhysicalComponents};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::component_sources::physical_component;
use crate::catalog_v3::referenced_definition_read::{self, Selection};
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;

#[path = "catalog_attachable_modifier.rs"]
mod attachable;
#[path = "attachable_effect.rs"]
mod attachable_effect;
#[path = "mining_laser_global_definition.rs"]
mod global;
#[path = "catalog_mining_laser.rs"]
mod laser;
#[path = "mining_configuration_modifiers.rs"]
mod modifiers;
#[path = "attachable_weapon_modifier.rs"]
mod weapon_modifier;

pub use attachable::AttachableModifier;
pub use attachable_effect::{
    AttachableEffect, FilterEffect, MiningEffect, ModifierLifetime, WeaponEffect,
};
pub use global::MiningLaserGlobalDefinition;
pub(crate) use laser::GLOBAL;
pub use laser::MiningLaser;
pub use modifiers::{FloatModifier, MiningFilter, MiningModifiers};
pub use weapon_modifier::{SalvageModifier, TractorModifier, WeaponModifier, WeaponModifierStats};

const MINING_LASER: ComponentKind = ComponentKind {
    source_type: laser::COMPONENT,
    inspection: ConfigurationInspection::MiningLaser,
};
const ATTACHABLE_MODIFIER: ComponentKind = ComponentKind {
    source_type: attachable::COMPONENT,
    inspection: ConfigurationInspection::AttachableModifier,
};

physical_component!(MiningLaserComponents, MiningLaser, MINING_LASER);
physical_component!(
    AttachableModifierComponents,
    AttachableModifier,
    ATTACHABLE_MODIFIER
);

/// Global references and physical mining and attachment configurations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MiningConfigurationTables<'source> {
    mining_lasers: CatalogTable<MiningLaser<'source>>,
    attachable_modifiers: CatalogTable<AttachableModifier<'source>>,
}

impl<'source> MiningConfigurationTables<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            mining_lasers: components.table(MINING_LASER, MiningLaser::read)?,
            attachable_modifiers: components
                .table(ATTACHABLE_MODIFIER, AttachableModifier::read)?,
        })
    }

    pub(super) fn globals(
        &self,
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<CatalogTable<MiningLaserGlobalDefinition<'source>>, CatalogError> {
        let linked = self
            .mining_lasers
            .rows
            .iter()
            .filter_map(MiningLaser::global_index)
            .collect();
        let definitions = referenced_definition_read::read(
            components.forge,
            components.projection.original(),
            &Selection {
                source_types: &[GLOBAL],
                linked: &linked,
                filter: components.options.filter,
                inspection: ConfigurationInspection::MiningLaserGlobal,
            },
            components.options.cancel,
            MiningLaserGlobalDefinition::read,
        )?;
        let rows = components.keep_definitions(definitions);
        Ok(CatalogTable::new(components.forge, &[GLOBAL], rows))
    }
}
