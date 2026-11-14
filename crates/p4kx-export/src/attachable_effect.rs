// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use super::modifiers::{MiningFilter, MiningModifiers};
use super::weapon_modifier::WeaponModifier;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The concrete authored effect of one attachable modifier slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub(super) enum AttachableEffect<'source> {
    MiningLaser(Box<MiningEffect<'source>>),
    MineableRock(Box<MiningEffect<'source>>),
    MiningFilter(Box<FilterEffect<'source>>),
    Weapon(Box<WeaponEffect<'source>>),
    Authored(UnitSource<'source>),
}

impl<'source> AttachableEffect<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "ItemMiningModifierParams" => {
                Self::MiningLaser(Box::new(MiningEffect::read(source, notes)?))
            }
            "ItemMineableRockModifierParams" => {
                Self::MineableRock(Box::new(MiningEffect::read(source, notes)?))
            }
            "MiningFilterItemModifierParams" => {
                Self::MiningFilter(Box::new(FilterEffect::read(source, notes)?))
            }
            "ItemWeaponModifiersParams" => {
                Self::Weapon(Box::new(WeaponEffect::read(source, notes)?))
            }
            _ => Self::Authored(UnitSource::read(source, [])?),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MiningEffect<'source> {
    modifier_lifetime: SourcedValue<ModifierLifetime<'source>>,
    mining_laser_modifier: SourcedValue<MiningModifiers<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> MiningEffect<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let modifier_lifetime =
            source.declared_pointer("modifierLifetime", notes, ModifierLifetime::read)?;
        let mining_laser_modifier = source.declared_class_to(
            "MiningLaserModifier",
            "MiningLaserModifiers",
            notes,
            MiningModifiers::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("modifierLifetime", modifier_lifetime.state()),
                ("MiningLaserModifier", mining_laser_modifier.state()),
            ],
        )?;
        Ok(Self {
            modifier_lifetime,
            mining_laser_modifier,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FilterEffect<'source> {
    modifier_lifetime: SourcedValue<ModifierLifetime<'source>>,
    filter_params: SourcedValue<MiningFilter<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> FilterEffect<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let modifier_lifetime =
            source.declared_pointer("modifierLifetime", notes, ModifierLifetime::read)?;
        let filter_params = source.declared_class_to(
            "filterParams",
            "MiningFilterParams",
            notes,
            MiningFilter::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("modifierLifetime", modifier_lifetime.state()),
                ("filterParams", filter_params.state()),
            ],
        )?;
        Ok(Self {
            modifier_lifetime,
            filter_params,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WeaponEffect<'source> {
    modifier_lifetime: SourcedValue<ModifierLifetime<'source>>,
    fire_action_index: SourcedValue<i32>,
    weapon_modifier: SourcedValue<WeaponModifier<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> WeaponEffect<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let modifier_lifetime =
            source.declared_pointer("modifierLifetime", notes, ModifierLifetime::read)?;
        let fire_action_index = source.int32("fireActionIndex", notes)?;
        let weapon_modifier = source.declared_class_to(
            "weaponModifier",
            "SWeaponModifierParams",
            notes,
            WeaponModifier::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("modifierLifetime", modifier_lifetime.state()),
                ("fireActionIndex", fire_action_index.state()),
                ("weaponModifier", weapon_modifier.state()),
            ],
        )?;
        Ok(Self {
            modifier_lifetime,
            fire_action_index,
            weapon_modifier,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ModifierLifetime<'source> {
    lifetime: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> ModifierLifetime<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let lifetime = source.single("lifetime", notes)?;
        let original = UnitSource::read(source, [("lifetime", lifetime.state())])?;
        Ok(Self { lifetime, original })
    }
}
