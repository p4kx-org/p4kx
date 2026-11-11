// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::authored_link::HealthLink;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const DAMAGE_TYPES: &[&str] = &["DamageBase", "DamageInfo", "DamageParams"];

/// The authored damage alternatives of one `DamageBase` declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum AuthoredDamage<'source> {
    Base,
    Info {
        value: Box<DamageInfo>,
    },
    Parameters {
        value: Box<DamageParameters<'source>>,
    },
}

impl<'source> AuthoredDamage<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "DamageBase" => Self::Base,
            "DamageInfo" => Self::Info {
                value: Box::new(DamageInfo::read(source, notes)?),
            },
            "DamageParams" => Self::Parameters {
                value: Box::new(DamageParameters::read(source, notes)?),
            },
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Exact original damage words by declared type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DamageInfo {
    physical: SourcedValue<ResourceFloat32>,
    energy: SourcedValue<ResourceFloat32>,
    distortion: SourcedValue<ResourceFloat32>,
    thermal: SourcedValue<ResourceFloat32>,
    biochemical: SourcedValue<ResourceFloat32>,
    stun: SourcedValue<ResourceFloat32>,
}

impl DamageInfo {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            physical: source.single("DamagePhysical", notes)?,
            energy: source.single("DamageEnergy", notes)?,
            distortion: source.single("DamageDistortion", notes)?,
            thermal: source.single("DamageThermal", notes)?,
            biochemical: source.single("DamageBiochemical", notes)?,
            stun: source.single("DamageStun", notes)?,
        })
    }
}

/// The original macro link and declared total.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DamageParameters<'source> {
    damage_macro: SourcedValue<HealthLink<'source>>,
    damage_total: SourcedValue<ResourceFloat32>,
}

impl<'source> DamageParameters<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_macro: source.link("damageMacro", notes)?,
            damage_total: source.single("damageTotal", notes)?,
        })
    }
}

/// Three original Single coordinates of an authored Vec3.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DamageVector {
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
    z: SourcedValue<ResourceFloat32>,
}

impl DamageVector {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            x: source.single("x", notes)?,
            y: source.single("y", notes)?,
            z: source.single("z", notes)?,
        })
    }
}
