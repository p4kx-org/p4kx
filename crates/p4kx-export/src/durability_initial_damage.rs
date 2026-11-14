// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::damage::{AuthoredDamage, DAMAGE_TYPES, DamageVector};
use super::link::HealthLink;
use super::observed::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// One authored initial-damage configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InitialDamage<'source> {
    random_seed: SourcedValue<i32>,
    max_damage_ratio: SourcedValue<ResourceFloat32>,
    min_hit_count: SourcedValue<i32>,
    max_hit_count: SourcedValue<i32>,
    min_hit_radius_fraction: SourcedValue<ResourceFloat32>,
    max_hit_radius_fraction: SourcedValue<ResourceFloat32>,
    hit_damage_variation_factor: SourcedValue<ResourceFloat32>,
    damage_map_damage_scale: SourcedValue<ResourceFloat32>,
    damage: SourcedValue<DamageSpecifier<'source>>,
    bounding_box_scale: SourcedValue<DamageVector>,
    damage_map_noise_strength: SourcedValue<DamageMapChannels>,
}

impl<'source> InitialDamage<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            random_seed: source.int32("RandomSeed", notes)?,
            max_damage_ratio: source.single("MaxDamageRatio", notes)?,
            min_hit_count: source.int32("MinHitCount", notes)?,
            max_hit_count: source.int32("MaxHitCount", notes)?,
            min_hit_radius_fraction: source.single("MinHitRadiusFraction", notes)?,
            max_hit_radius_fraction: source.single("MaxHitRadiusFraction", notes)?,
            hit_damage_variation_factor: source.single("HitDamageVariationFactor", notes)?,
            damage_map_damage_scale: source.single("DamageMapDamageScale", notes)?,
            damage: source.owned(
                "Damage",
                &[
                    "SInitialDamageSpecifierBase",
                    "SInitialDamageSpecifierFixed",
                    "SInitialDamageSpecifierFraction",
                ],
                notes,
                DamageSpecifier::read,
            )?,
            bounding_box_scale: source.owned(
                "BoundingBoxScale",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
            damage_map_noise_strength: source.owned(
                "DamageMapNoiseStrength",
                &["DamageMapChannels"],
                notes,
                DamageMapChannels::read,
            )?,
        })
    }
}

/// The concrete original `InitialDamage` damage specifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum DamageSpecifier<'source> {
    Base,
    Fixed { value: Box<FixedDamage<'source>> },
    Fraction { value: Box<FractionDamage<'source>> },
}

impl<'source> DamageSpecifier<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SInitialDamageSpecifierBase" => Self::Base,
            "SInitialDamageSpecifierFixed" => Self::Fixed {
                value: Box::new(FixedDamage {
                    damage: source.owned("Damage", DAMAGE_TYPES, notes, AuthoredDamage::read)?,
                }),
            },
            "SInitialDamageSpecifierFraction" => Self::Fraction {
                value: Box::new(FractionDamage::read(source, notes)?),
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FixedDamage<'source> {
    damage: SourcedValue<AuthoredDamage<'source>>,
}

/// The authored damage macro and fraction, without applying either.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FractionDamage<'source> {
    damage_macro: SourcedValue<HealthLink<'source>>,
    health_fraction: SourcedValue<ResourceFloat32>,
}

impl<'source> FractionDamage<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_macro: source.link("damageMacro", notes)?,
            health_fraction: source.single("HealthFraction", notes)?,
        })
    }
}

/// The original damage-map channel words.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DamageMapChannels {
    thickness: SourcedValue<ResourceFloat32>,
    deformation: SourcedValue<ResourceFloat32>,
    burn: SourcedValue<ResourceFloat32>,
    temperature: SourcedValue<ResourceFloat32>,
}

impl DamageMapChannels {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            thickness: source.single("thickness", notes)?,
            deformation: source.single("deformation", notes)?,
            burn: source.single("burn", notes)?,
            temperature: source.single("temperature", notes)?,
        })
    }
}
