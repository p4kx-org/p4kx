// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_entity_component_read::{
    self as entity_component, ComponentKind, ComponentRead, ComponentSelection,
};
use super::catalog_mission_board::{self, MissionBoard};
use super::catalog_radar_options::RadarOptionsConfiguration;
use super::catalog_resource_network_configuration::ResourceNetworkConfiguration;
use super::catalog_resource_network_read::{self, GameModeRead};
use super::jump_drive_definitions::{self, JumpDriveDefinitions};
use super::referenced_definition_read::Definitions;
use super::weapon_action_recoil_definitions::{self, WeaponActionRecoilDefinition};
use super::weapon_procedural_definitions::{self, WeaponProceduralTables};
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::collected::Collected;
use crate::contract_recovery::{ItemRecoveryEconomy, ItemRecoveryOverrides};
use crate::export_field_inspection::ConfigurationInspection;
use crate::recovery::{self, ItemRecoveryPolicy, Recovery};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions};
use entity_animation_controller::EntityAnimationController;
use loot_generation_component::LootGenerationComponent;

#[path = "consumable_tables.rs"]
mod consumables;
#[path = "catalog_entity_animation_controller.rs"]
mod entity_animation_controller;
#[path = "flight_configuration_tables.rs"]
pub(in crate::catalog_v3) mod flight_configuration_tables;
#[path = "hackable_configuration.rs"]
mod hackable_configuration;
#[path = "catalog_jump_point.rs"]
mod jump_point;
#[path = "jump_point_globals.rs"]
mod jump_point_globals;
#[path = "loot_generation_component.rs"]
mod loot_generation_component;
#[path = "mining_configuration_tables.rs"]
mod mining_configurations;
#[path = "catalog_ordered_curve.rs"]
pub(super) mod ordered_curve;
#[path = "turret_health.rs"]
mod turret_health;
#[path = "worn_gear_tables.rs"]
mod worn_gear;
use consumables::{ConsumableConfigurations, ConsumableTables};
pub(crate) use consumables::{
    ConsumableDefinitions, SUBTYPE as CONSUMABLE_SUBTYPE, TYPE as CONSUMABLE_TYPE,
};
pub use consumables::{ConsumableSubtype, ConsumableType};

pub use super::authored_effect_resource::Rtpc;
pub use super::catalog_durability::resistance::{DurabilityResistanceEntry, ResistanceSet};
pub use consumables::{AuthoredList, ConsumableEffect, EffectParameters, GasMass, Override};
use flight_configuration_tables::FlightConfigurationTables;
use hackable_configuration::HackableConfiguration;
use jump_point::JumpPointConfiguration;
use jump_point_globals::GlobalJumpConfiguration;
pub(crate) use mining_configurations::GLOBAL as MINING_LASER_GLOBAL;
use mining_configurations::MiningConfigurationTables;
pub use mining_configurations::MiningLaserGlobalDefinition;
pub use ordered_curve::OrderedCurve;
pub use turret_health::TurretHealthModifier;
pub(crate) use turret_health::{HEALTH_MODIFIER, observe_modifier_types};
use turret_health::{TurretHealth, TurretTables};
pub(crate) use worn_gear::MACRO as DAMAGE_RESISTANCE_MACRO;
use worn_gear::WornGearTables;
pub use worn_gear::{DamageResistanceMacro, MacroBody};

/// Independently scoped configurations with their physical owners.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct IndependentConfigurationTables<'source> {
    item_recovery_economy: CatalogTable<ItemRecoveryEconomy<'source>>,
    item_recovery_overrides: CatalogTable<ItemRecoveryOverrides>,
    item_recovery_policy: CatalogTable<ItemRecoveryPolicy<'source>>,
    resource_network_parameters: CatalogTable<ResourceNetworkConfiguration<'source>>,
    radar_options: CatalogTable<RadarOptionsConfiguration<'source>>,
    mission_boards: CatalogTable<MissionBoard<'source>>,
    entity_animation_controllers: CatalogTable<EntityAnimationController<'source>>,
    loot_generation: CatalogTable<LootGenerationComponent<'source>>,
    hackable_devices: CatalogTable<HackableConfiguration<'source>>,
    turret_health: CatalogTable<TurretHealth<'source>>,
    #[serde(flatten)]
    pub(crate) flight: FlightConfigurationTables<'source>,
    #[serde(flatten)]
    jump_drive_definitions: JumpDriveDefinitions<'source>,
    jump_point_configurations: CatalogTable<JumpPointConfiguration<'source>>,
    global_jump_point_configurations: CatalogTable<GlobalJumpConfiguration<'source>>,
    #[serde(flatten)]
    worn_gear: WornGearTables<'source>,
    #[serde(flatten)]
    consumables: ConsumableConfigurations<'source>,
    #[serde(flatten)]
    mining_configurations: MiningConfigurationTables<'source>,
}

pub(crate) struct IndependentConfigurations<'source> {
    pub(crate) tables: IndependentConfigurationTables<'source>,
    pub(crate) weapon_action_recoil_definitions:
        CatalogTable<WeaponActionRecoilDefinition<'source>>,
    pub(crate) weapon_procedural: WeaponProceduralTables<'source>,
    pub(crate) damage_resistance_macros: CatalogTable<DamageResistanceMacro<'source>>,
    pub(crate) turret_health_modifiers: CatalogTable<TurretHealthModifier<'source>>,
    pub(crate) consumable_definitions: ConsumableDefinitions<'source>,
    pub(crate) mining_laser_globals: CatalogTable<MiningLaserGlobalDefinition<'source>>,
    pub(crate) recovery_numbers: Vec<CapturedNumber>,
    pub(crate) configuration_numbers: Vec<CapturedNumber>,
}

/// Collects source evidence and output rows for the global configuration tables.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'source>,
    jump_owners: &HashSet<u32>,
) -> Result<IndependentConfigurations<'source>, CatalogError> {
    let (recovery, recovery_numbers) = read_recovery(forge, projection, options, collected)?;
    let (game_mode, mut configuration_numbers) =
        read_game_mode(forge, projection, options, collected)?;
    let mut components = PhysicalComponents {
        forge,
        projection,
        options,
        collected,
        numbers: &mut configuration_numbers,
    };
    let jump_drive_definitions = components.jump_definitions()?;
    let flight = FlightConfigurationTables::read(&mut components)?;
    let weapon_action_recoil_definitions = components.weapon_recoil()?;
    let weapon_procedural = components.weapon_procedural()?;
    let mission_boards = components.table(MISSION_BOARDS, MissionBoard::read)?;
    let entity_animation_controllers =
        components.table(ANIMATION_CONTROLLERS, EntityAnimationController::read)?;
    let loot_generation = components.table(LOOT_GENERATION, LootGenerationComponent::read)?;
    let hackable_devices = components.table(HACKABLE_DEVICES, HackableConfiguration::read)?;
    let jump_points = components.jump_points(jump_owners)?;
    let turret = TurretTables::read(&mut components)?;
    let worn_gear = WornGearTables::read(&mut components)?;
    let damage_resistance_macros = worn_gear.resistance_macros(&mut components)?;
    let consumables = ConsumableTables::read(&mut components)?;
    let mining_configurations = MiningConfigurationTables::read(&mut components)?;
    let mining_laser_globals = mining_configurations.globals(&mut components)?;
    Ok(IndependentConfigurations {
        tables: IndependentConfigurationTables {
            item_recovery_economy: recovery.economy,
            item_recovery_overrides: recovery.overrides,
            item_recovery_policy: recovery.policy,
            resource_network_parameters: game_mode.resource_network_parameters,
            radar_options: game_mode.radar_options,
            mission_boards,
            entity_animation_controllers,
            loot_generation,
            hackable_devices,
            turret_health: turret.health,
            flight,
            jump_drive_definitions,
            jump_point_configurations: jump_points.local,
            global_jump_point_configurations: jump_points.global,
            worn_gear,
            consumables: consumables.configurations,
            mining_configurations,
        },
        weapon_action_recoil_definitions,
        damage_resistance_macros,
        turret_health_modifiers: turret.modifiers,
        consumable_definitions: consumables.definitions,
        mining_laser_globals,
        weapon_procedural,
        recovery_numbers,
        configuration_numbers,
    })
}

struct RecoveryTables<'source> {
    economy: CatalogTable<ItemRecoveryEconomy<'source>>,
    overrides: CatalogTable<ItemRecoveryOverrides>,
    policy: CatalogTable<ItemRecoveryPolicy<'source>>,
}

fn read_recovery<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'source>,
) -> Result<(RecoveryTables<'source>, Vec<CapturedNumber>), CatalogError> {
    let Recovery {
        economy,
        overrides,
        policy,
        numbers,
        failures,
        ledger,
    } = recovery::read_recovery(forge, projection, options.filter, options.cancel)?;
    collected.ledger.merge(ledger);
    collected.add_failures(failures);
    Ok((
        RecoveryTables {
            economy,
            overrides,
            policy,
        },
        numbers,
    ))
}

struct GameModeTables<'source> {
    resource_network_parameters: CatalogTable<ResourceNetworkConfiguration<'source>>,
    radar_options: CatalogTable<RadarOptionsConfiguration<'source>>,
}

struct JumpPointTables<'source> {
    local: CatalogTable<JumpPointConfiguration<'source>>,
    global: CatalogTable<GlobalJumpConfiguration<'source>>,
}

fn read_game_mode<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'source>,
) -> Result<(GameModeTables<'source>, Vec<CapturedNumber>), CatalogError> {
    let GameModeRead {
        resource_network_parameters,
        radar_options,
        numbers,
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
    Ok((
        GameModeTables {
            resource_network_parameters,
            radar_options,
        },
        numbers,
    ))
}

const ANIMATION_CONTROLLERS: ComponentKind = ComponentKind {
    source_type: entity_animation_controller::COMPONENT,
    inspection: ConfigurationInspection::EntityAnimationController,
};
const MISSION_BOARDS: ComponentKind = ComponentKind {
    source_type: catalog_mission_board::COMPONENT,
    inspection: ConfigurationInspection::MissionBoard,
};
const LOOT_GENERATION: ComponentKind = ComponentKind {
    source_type: loot_generation_component::COMPONENT,
    inspection: ConfigurationInspection::LootGeneration,
};
const JUMP_POINTS: ComponentKind = ComponentKind {
    source_type: jump_point::COMPONENT,
    inspection: ConfigurationInspection::JumpPoint,
};
const HACKABLE_DEVICES: ComponentKind = ComponentKind {
    source_type: hackable_configuration::COMPONENT,
    inspection: ConfigurationInspection::HackableDevice,
};

struct PhysicalComponents<'options, 'input, 'source> {
    forge: &'source DataForge<'source>,
    projection: &'options CatalogProjection<'input, 'source>,
    options: &'options CatalogOptions<'input>,
    collected: &'options mut Collected<'source>,
    numbers: &'options mut Vec<CapturedNumber>,
}

impl<'source> PhysicalComponents<'_, '_, 'source> {
    fn keep_definitions<Row>(&mut self, definitions: Definitions<'source, Row>) -> Vec<Row> {
        self.numbers.extend(definitions.numbers);
        self.collected.ledger.merge(definitions.ledger);
        self.collected.add_failures(definitions.failures);
        definitions.rows.into_iter().map(|(_, row)| row).collect()
    }
    fn jump_points(
        &mut self,
        owners: &HashSet<u32>,
    ) -> Result<JumpPointTables<'source>, CatalogError> {
        let local = self.selected_table(JUMP_POINTS, JumpPointConfiguration::read, Some(owners))?;
        let global = jump_point_globals::read(
            self.forge,
            self.projection.original(),
            self.options.filter,
            self.options.cancel,
        )?;
        self.numbers.extend(global.numbers);
        self.collected.ledger.merge(global.ledger);
        self.collected.add_failures(global.failures);
        Ok(JumpPointTables {
            local,
            global: jump_point_globals::table(self.forge, global.rows),
        })
    }

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
        Ok(weapon_action_recoil_definitions::table(
            self.forge,
            recoil.rows.into_iter().map(|(_, row)| row).collect(),
        ))
    }

    fn weapon_procedural(&mut self) -> Result<WeaponProceduralTables<'source>, CatalogError> {
        let (tables, evidence) = weapon_procedural_definitions::read(
            self.forge,
            self.projection.original(),
            self.options.filter,
            self.options.cancel,
            &self.collected.item_components.weapon_actions,
        )?;
        self.numbers.extend(evidence.numbers);
        self.collected.ledger.merge(evidence.ledger);
        self.collected.add_failures(evidence.failures);
        Ok(tables)
    }

    fn table<Row>(
        &mut self,
        component: ComponentKind,
        project: entity_component::Projection<'source, Row>,
    ) -> Result<CatalogTable<Row>, CatalogError> {
        self.selected_table(component, project, None)
    }

    fn selected_table<Row>(
        &mut self,
        component: ComponentKind,
        project: entity_component::Projection<'source, Row>,
        dependencies: Option<&HashSet<u32>>,
    ) -> Result<CatalogTable<Row>, CatalogError> {
        let ComponentRead {
            rows,
            numbers,
            ledger,
            failures,
        } = entity_component::read(
            self.forge,
            self.projection.original(),
            &ComponentSelection::export(self.options.filter, dependencies),
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
