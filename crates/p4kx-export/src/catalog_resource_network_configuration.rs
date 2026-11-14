// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
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
pub(crate) struct ResourceNetworkConfiguration<'source> {
    source: CanonicalRecordHeader<'source>,
    game_mode_id: SourcedValue<&'source str>,
    params: SourcedValue<ModeParameters<'source>>,
}

impl<'source> ResourceNetworkConfiguration<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        mode: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source,
            game_mode_id: mode.borrowed_enum_to("id", "EGameModeId", notes)?,
            params: mode.strong_pointer_family_to(
                PARAMS,
                "SIParamsModule",
                notes,
                ModeParameters::read,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ModeParameters<'source> {
    source_type: SourcedValue<&'source str>,
    resource_network_params: SourcedValue<ResourceNetworkParameters<'source>>,
}

impl<'source> ModeParameters<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: source.source_type_value()?,
            resource_network_params: source.declared_class_to(
                NETWORK,
                NETWORK_TYPE,
                notes,
                ResourceNetworkParameters::read,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ResourceNetworkParameters<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    mode: ResourceModeFlags,
    #[serde(flatten)]
    destruction: ShipDestructionRules,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceNetworkParameters<'source> {
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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ResourceModeFlags {
    enable_item_resource_network: SourcedValue<bool>,
    enable_salvage_item_repair: SourcedValue<bool>,
    enable_misfires: SourcedValue<bool>,
    enable_critical_misfires: SourcedValue<bool>,
    #[serde(rename = "doorsIgnoreRN")]
    doors_ignore_resource_network: SourcedValue<bool>,
    enable_experimental_armor_penetration: SourcedValue<bool>,
}

impl ResourceModeFlags {
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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ShipDestructionRules {
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
