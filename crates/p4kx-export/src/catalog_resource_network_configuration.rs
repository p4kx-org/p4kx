// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_item_components::source_getters;
use super::catalog_radar_options::{RadarModeParameters, RadarOptionsConfiguration};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const GAME_MODE: &str = "GameMode";
pub(super) const PARAMS: &str = "params";
pub(super) const NETWORK: &str = "resourceNetworkParams";
const NETWORK_TYPE: &str = "ResourceNetworkParams";

/// Resource-network settings from a physically identified game-mode Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceNetworkConfiguration<'source> {
    source: CanonicalRecordHeader<'source>,
    game_mode_id: SourcedValue<&'source str>,
    params: SourcedValue<ModeParameters<'source>>,
}

impl<'source> ResourceNetworkConfiguration<'source> {
    source_getters! {
        /// Borrows the original physical game-mode Record header.
        source: CanonicalRecordHeader<'source>,
        /// Borrows the authored game-mode identifier.
        game_mode_id: SourcedValue<&'source str>,
        /// Borrows the original parameter pointer state and resource-network branch.
        params: SourcedValue<ModeParameters<'source>>,
    }

    const fn new(
        source: CanonicalRecordHeader<'source>,
        game_mode_id: SourcedValue<&'source str>,
        params: SourcedValue<ModeParameters<'source>>,
    ) -> Self {
        Self {
            source,
            game_mode_id,
            params,
        }
    }
}

pub(super) struct GameModeConfigurations<'source> {
    pub(super) network: ResourceNetworkConfiguration<'source>,
    pub(super) radar: RadarOptionsConfiguration<'source>,
}

impl<'source> GameModeConfigurations<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        mode: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let GameModeFields {
            game_mode_id,
            params,
        } = read_mode(mode, notes, |params, source_type, notes| {
            Ok((
                ModeParameters::read(params, source_type.clone(), notes)?,
                RadarModeParameters::read(params, source_type, notes)?,
            ))
        })?;
        let (network, radar) = params.unzip();
        Ok(Self {
            network: ResourceNetworkConfiguration::new(
                source.clone(),
                game_mode_id.clone(),
                network,
            ),
            radar: RadarOptionsConfiguration::new(source, game_mode_id, radar),
        })
    }
}

pub(super) fn read_network<'source>(
    source: CanonicalRecordHeader<'source>,
    mode: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceNetworkConfiguration<'source>, CatalogError> {
    let GameModeFields {
        game_mode_id,
        params,
    } = read_mode(mode, notes, ModeParameters::read)?;
    Ok(ResourceNetworkConfiguration::new(
        source,
        game_mode_id,
        params,
    ))
}

pub(super) fn read_radar<'source>(
    source: CanonicalRecordHeader<'source>,
    mode: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<RadarOptionsConfiguration<'source>, CatalogError> {
    let GameModeFields {
        game_mode_id,
        params,
    } = read_mode(mode, notes, RadarModeParameters::read)?;
    Ok(RadarOptionsConfiguration::new(source, game_mode_id, params))
}

struct GameModeFields<'source, Row> {
    game_mode_id: SourcedValue<&'source str>,
    params: SourcedValue<Row>,
}

fn read_mode<'source, Row>(
    mode: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
    read: impl FnOnce(
        Observed<'_, '_, '_, 'source>,
        SourcedValue<&'source str>,
        &mut Vec<FieldError>,
    ) -> Result<Row, CatalogError>,
) -> Result<GameModeFields<'source, Row>, CatalogError> {
    let game_mode_id = mode.borrowed_enum_to("id", "EGameModeId", notes)?;
    let params =
        mode.strong_pointer_family_to(PARAMS, "SIParamsModule", notes, |params, notes| {
            let source_type = params.source_type_value()?;
            read(params, source_type, notes)
        })?;
    Ok(GameModeFields {
        game_mode_id,
        params,
    })
}

/// Resource-network branch of the original owned game-mode parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ModeParameters<'source> {
    source_type: SourcedValue<&'source str>,
    resource_network_params: SourcedValue<ResourceNetworkParameters<'source>>,
}

impl<'source> ModeParameters<'source> {
    source_getters! {
        /// Borrows the original concrete game-mode parameter class.
        source_type: SourcedValue<&'source str>,
        /// Borrows the typed resource-network declaration and its source state.
        resource_network_params: SourcedValue<ResourceNetworkParameters<'source>>,
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        source_type: SourcedValue<&'source str>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type,
            resource_network_params: source.declared_class_to(
                NETWORK,
                NETWORK_TYPE,
                notes,
                ResourceNetworkParameters::read,
            )?,
        })
    }
}

/// Original network flags, ship destruction rules and authored extensions.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceNetworkParameters<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    mode: ResourceModeFlags,
    #[serde(flatten)]
    destruction: ShipDestructionRules,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceNetworkParameters<'source> {
    source_getters! {
        /// Borrows the original resource-network parameter class.
        source_type: SourcedValue<&'source str>,
        /// Borrows the original resource-network flags.
        mode: ResourceModeFlags,
        /// Borrows the authored ship destruction rules.
        destruction: ShipDestructionRules,
        /// Borrows the remaining authored fields.
        additional_fields: SourceArenaOutput<'source>,
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mode = ResourceModeFlags::read(source, notes)?;
        let destruction = ShipDestructionRules::read(source, notes)?;
        let additional_fields = source
            .authored_members_except(mode.projected().into_iter().chain(destruction.projected()))?;
        Ok(Self {
            source_type: source.source_type_value()?,
            mode,
            destruction,
            additional_fields,
        })
    }
}

/// Original resource-network flags for a game mode.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceModeFlags {
    enable_item_resource_network: SourcedValue<bool>,
    enable_salvage_item_repair: SourcedValue<bool>,
    enable_misfires: SourcedValue<bool>,
    enable_critical_misfires: SourcedValue<bool>,
    #[serde(rename = "doorsIgnoreRN")]
    doors_ignore_resource_network: SourcedValue<bool>,
    enable_experimental_armor_penetration: SourcedValue<bool>,
}

impl ResourceModeFlags {
    source_getters! {
        /// Borrows the authored item network flag.
        enable_item_resource_network: SourcedValue<bool>,
        /// Borrows the authored salvage and repair flag.
        enable_salvage_item_repair: SourcedValue<bool>,
        /// Borrows the authored misfire flag.
        enable_misfires: SourcedValue<bool>,
        /// Borrows the authored critical misfire flag.
        enable_critical_misfires: SourcedValue<bool>,
        /// Borrows the authored door exclusion flag.
        doors_ignore_resource_network: SourcedValue<bool>,
        /// Borrows the authored experimental armor penetration flag.
        enable_experimental_armor_penetration: SourcedValue<bool>,
    }

    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            enable_item_resource_network: source.boolean("enableItemResourceNetwork", notes)?,
            enable_salvage_item_repair: source.boolean("enableSalvageItemRepair", notes)?,
            enable_misfires: source.boolean("enableMisfires", notes)?,
            enable_critical_misfires: source.boolean("enableCriticalMisfires", notes)?,
            doors_ignore_resource_network: source.boolean("doorsIgnoreRN", notes)?,
            enable_experimental_armor_penetration: source
                .boolean("enableExperimentalArmorPenetration", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            (
                "enableItemResourceNetwork",
                self.enable_item_resource_network.state(),
            ),
            (
                "enableSalvageItemRepair",
                self.enable_salvage_item_repair.state(),
            ),
            ("enableMisfires", self.enable_misfires.state()),
            (
                "enableCriticalMisfires",
                self.enable_critical_misfires.state(),
            ),
            ("doorsIgnoreRN", self.doors_ignore_resource_network.state()),
            (
                "enableExperimentalArmorPenetration",
                self.enable_experimental_armor_penetration.state(),
            ),
        ]
    }
}

/// Original damage propagation and destruction thresholds for ships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipDestructionRules {
    #[serde(rename = "shipPassDestroyedPartDamageToComponent")]
    pass_part_damage_to_component: SourcedValue<bool>,
    #[serde(rename = "shipPassDestroyedPartDamageToChild")]
    pass_part_damage_to_child: SourcedValue<bool>,
    #[serde(rename = "shipDestructionBufferMultiplier")]
    buffer_multiplier: SourcedValue<ResourceFloat32>,
    #[serde(rename = "shipDestructionHealthThreshold")]
    health_threshold: SourcedValue<ResourceFloat32>,
}

impl ShipDestructionRules {
    source_getters! {
        /// Borrows the authored component damage propagation flag.
        pass_part_damage_to_component: SourcedValue<bool>,
        /// Borrows the authored child damage propagation flag.
        pass_part_damage_to_child: SourcedValue<bool>,
        /// Borrows the original destruction buffer multiplier.
        buffer_multiplier: SourcedValue<ResourceFloat32>,
        /// Borrows the original destruction health threshold.
        health_threshold: SourcedValue<ResourceFloat32>,
    }

    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            pass_part_damage_to_component: source
                .boolean("shipPassDestroyedPartDamageToComponent", notes)?,
            pass_part_damage_to_child: source
                .boolean("shipPassDestroyedPartDamageToChild", notes)?,
            buffer_multiplier: source.single("shipDestructionBufferMultiplier", notes)?,
            health_threshold: source.single("shipDestructionHealthThreshold", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 4] {
        [
            (
                "shipPassDestroyedPartDamageToComponent",
                self.pass_part_damage_to_component.state(),
            ),
            (
                "shipPassDestroyedPartDamageToChild",
                self.pass_part_damage_to_child.state(),
            ),
            (
                "shipDestructionBufferMultiplier",
                self.buffer_multiplier.state(),
            ),
            (
                "shipDestructionHealthThreshold",
                self.health_threshold.state(),
            ),
        ]
    }
}
