// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original weapon changes carried by one attachable modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WeaponModifier<'source> {
    weapon_stats: SourcedValue<WeaponModifierStats<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> WeaponModifier<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let weapon_stats = source.declared_class_to(
            "weaponStats",
            "SWeaponStats",
            notes,
            WeaponModifierStats::read,
        )?;
        let original = UnitSource::read(source, [("weaponStats", weapon_stats.state())])?;
        Ok(Self {
            weapon_stats,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct WeaponModifierStats<'source> {
    damage_multiplier: SourcedValue<ResourceFloat32>,
    salvage_modifier: SourcedValue<SalvageModifier<'source>>,
    tractor_modifier: SourcedValue<TractorModifier<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> WeaponModifierStats<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let damage_multiplier = source.single("damageMultiplier", notes)?;
        let salvage_modifier = source.declared_class_to(
            "salvageModifier",
            "SSalvageModifier",
            notes,
            SalvageModifier::read,
        )?;
        let tractor_modifier = source.declared_class_to(
            "tractorModifier",
            "STractorModifier",
            notes,
            TractorModifier::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("damageMultiplier", damage_multiplier.state()),
                ("salvageModifier", salvage_modifier.state()),
                ("tractorModifier", tractor_modifier.state()),
            ],
        )?;
        Ok(Self {
            damage_multiplier,
            salvage_modifier,
            tractor_modifier,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SalvageModifier<'source> {
    radius_multiplier: SourcedValue<ResourceFloat32>,
    salvage_speed_multiplier: SourcedValue<ResourceFloat32>,
    extraction_efficiency: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> SalvageModifier<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let radius_multiplier = source.single("radiusMultiplier", notes)?;
        let salvage_speed_multiplier = source.single("salvageSpeedMultiplier", notes)?;
        let extraction_efficiency = source.single("extractionEfficiency", notes)?;
        let original = UnitSource::read(
            source,
            [
                ("radiusMultiplier", radius_multiplier.state()),
                ("salvageSpeedMultiplier", salvage_speed_multiplier.state()),
                ("extractionEfficiency", extraction_efficiency.state()),
            ],
        )?;
        Ok(Self {
            radius_multiplier,
            salvage_speed_multiplier,
            extraction_efficiency,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TractorModifier<'source> {
    force_multiplier: SourcedValue<ResourceFloat32>,
    full_strength_dist_multiplier: SourcedValue<ResourceFloat32>,
    max_dist_multiplier: SourcedValue<ResourceFloat32>,
    max_volume_multiplier: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> TractorModifier<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let force_multiplier = source.single("forceMultiplier", notes)?;
        let full_strength_dist_multiplier = source.single("fullStrengthDistMultiplier", notes)?;
        let max_dist_multiplier = source.single("maxDistMultiplier", notes)?;
        let max_volume_multiplier = source.single("maxVolumeMultiplier", notes)?;
        let original = UnitSource::read(
            source,
            [
                ("forceMultiplier", force_multiplier.state()),
                (
                    "fullStrengthDistMultiplier",
                    full_strength_dist_multiplier.state(),
                ),
                ("maxDistMultiplier", max_dist_multiplier.state()),
                ("maxVolumeMultiplier", max_volume_multiplier.state()),
            ],
        )?;
        Ok(Self {
            force_multiplier,
            full_strength_dist_multiplier,
            max_dist_multiplier,
            max_volume_multiplier,
            original,
        })
    }
}
