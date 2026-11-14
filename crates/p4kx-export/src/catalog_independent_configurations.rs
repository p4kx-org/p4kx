// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_entity_component_read::{
    self as entity_component, ComponentKind, ComponentRead,
};
use super::catalog_mission_board::{self, MissionBoard};
use super::catalog_resource_network_configuration::ResourceNetworkConfiguration;
use super::catalog_resource_network_read::{self, ResourceNetworkRead};
use super::catalog_weapon_actions::RECOIL_DEFINITION;
use super::jump_drive_definitions::{self, JumpDriveDefinitions};
use super::weapon_action_recoil_definitions::{self, WeaponActionRecoilDefinition};
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::collected::Collected;
use crate::contract_recovery::{ItemRecoveryEconomy, ItemRecoveryOverrides};
use crate::export_field_inspection::ConfigurationInspection;
use crate::recovery::{self, ItemRecoveryPolicy, Recovery};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions};
use loot_generation_component::LootGenerationComponent;

#[path = "loot_generation_component.rs"]
mod loot_generation_component;

/// Independently scoped configurations with their physical owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct IndependentConfigurationTables<'source> {
    item_recovery_economy: CatalogTable<ItemRecoveryEconomy<'source>>,
    item_recovery_overrides: CatalogTable<ItemRecoveryOverrides>,
    item_recovery_policy: CatalogTable<ItemRecoveryPolicy<'source>>,
    resource_network_parameters: CatalogTable<ResourceNetworkConfiguration<'source>>,
    mission_boards: CatalogTable<MissionBoard<'source>>,
    loot_generation: CatalogTable<LootGenerationComponent<'source>>,
    #[serde(flatten)]
    jump_drive_definitions: JumpDriveDefinitions<'source>,
}

pub(crate) struct IndependentConfigurations<'source> {
    pub(crate) tables: IndependentConfigurationTables<'source>,
    pub(crate) weapon_action_recoil_definitions:
        CatalogTable<WeaponActionRecoilDefinition<'source>>,
    pub(crate) recovery_numbers: Vec<CapturedNumber>,
    pub(crate) configuration_numbers: Vec<CapturedNumber>,
}

/// Collects source evidence and output rows for the global configuration tables.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'source>,
) -> Result<IndependentConfigurations<'source>, CatalogError> {
    let Recovery {
        economy: item_recovery_economy,
        overrides: item_recovery_overrides,
        policy: item_recovery_policy,
        numbers: recovery_numbers,
        failures,
        ledger,
    } = recovery::read_recovery(forge, projection, options.filter, options.cancel)?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    let ResourceNetworkRead {
        table: resource_network_parameters,
        numbers: mut configuration_numbers,
        failures,
        ledger,
    } = catalog_resource_network_read::read(
        forge,
        projection.original(),
        options.filter,
        options.cancel,
    )?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    let mut components = PhysicalComponents {
        forge,
        projection,
        options,
        collected,
        numbers: &mut configuration_numbers,
    };
    let jump_drive_definitions = components.jump_definitions()?;
    let weapon_action_recoil_definitions = components.weapon_recoil()?;
    let mission_boards = components.table(MISSION_BOARDS, MissionBoard::read)?;
    let loot_generation = components.table(LOOT_GENERATION, LootGenerationComponent::read)?;
    Ok(IndependentConfigurations {
        tables: IndependentConfigurationTables {
            item_recovery_economy,
            item_recovery_overrides,
            item_recovery_policy,
            resource_network_parameters,
            mission_boards,
            loot_generation,
            jump_drive_definitions,
        },
        weapon_action_recoil_definitions,
        recovery_numbers,
        configuration_numbers,
    })
}

const MISSION_BOARDS: ComponentKind = ComponentKind {
    source_type: catalog_mission_board::COMPONENT,
    inspection: ConfigurationInspection::MissionBoard,
};
const LOOT_GENERATION: ComponentKind = ComponentKind {
    source_type: loot_generation_component::COMPONENT,
    inspection: ConfigurationInspection::LootGeneration,
};

struct PhysicalComponents<'options, 'input, 'source> {
    forge: &'source DataForge<'source>,
    projection: &'options CatalogProjection<'input, 'source>,
    options: &'options CatalogOptions<'input>,
    collected: &'options mut Collected<'source>,
    numbers: &'options mut Vec<CapturedNumber>,
}

impl<'source> PhysicalComponents<'_, '_, 'source> {
    fn jump_definitions(&mut self) -> Result<JumpDriveDefinitions<'source>, CatalogError> {
        let jump = jump_drive_definitions::read(
            self.forge,
            self.projection.original(),
            self.options.filter,
            self.options.cancel,
            &self.collected.item_components.jump_drives,
        )?;
        self.numbers.extend(jump.numbers);
        self.collected.ledger.merge(jump.ledger);
        self.collected.add_failures(jump.failures);
        Ok(jump.tables)
    }

    fn weapon_recoil(
        &mut self,
    ) -> Result<CatalogTable<WeaponActionRecoilDefinition<'source>>, CatalogError> {
        let recoil = weapon_action_recoil_definitions::read(
            self.forge,
            self.projection.original(),
            self.options.filter,
            self.options.cancel,
            &self.collected.item_components.weapon_actions,
        )?;
        self.numbers.extend(recoil.numbers);
        self.collected.ledger.merge(recoil.ledger);
        self.collected.add_failures(recoil.failures);
        Ok(CatalogTable::new(
            self.forge,
            &[RECOIL_DEFINITION],
            recoil.rows.into_iter().map(|(_, row)| row).collect(),
        ))
    }

    fn table<Row>(
        &mut self,
        component: ComponentKind,
        project: entity_component::Projection<'source, Row>,
    ) -> Result<CatalogTable<Row>, CatalogError> {
        let ComponentRead {
            rows,
            numbers,
            ledger,
            failures,
        } = entity_component::read(
            self.forge,
            self.projection.original(),
            self.options.filter,
            self.options.cancel,
            &component,
            project,
        )?;
        self.collected.ledger.merge(ledger);
        self.collected.add_failures(failures);
        self.numbers.extend(numbers);
        Ok(CatalogTable::new(
            self.forge,
            &[component.source_type],
            rows,
        ))
    }
}
