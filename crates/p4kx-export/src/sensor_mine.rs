// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_damage::DamageVector;
use super::authored_effect_resource::ParticleResource;
use super::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

const TRIGGER_TYPES: &[&str] = &[
    "SSensorMineTriggerType",
    "SSensorMineLaserTrigger",
    "SSensorMineProximityTrigger",
];

/// The authored mine sensor, its original visual and selected trigger form.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SensorMine {
    friendly_fire: SourcedValue<bool>,
    laser_vfx: SourcedValue<ParticleResource>,
    trigger_type: SourcedValue<SensorMineTrigger>,
}

impl SensorMine {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            friendly_fire: source.boolean("FriendlyFire", notes)?,
            laser_vfx: source.inline_class(
                "LaserVFX",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            trigger_type: source.strong_pointer(
                "TriggerType",
                TRIGGER_TYPES,
                notes,
                SensorMineTrigger::read,
            )?,
        })
    }
}

/// The actual owned trigger type admitted at the mine's `TriggerType` slot.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum SensorMineTrigger {
    Base,
    Laser { value: Box<LaserTrigger> },
    Proximity { value: Box<ProximityTrigger> },
}

impl SensorMineTrigger {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SSensorMineTriggerType" => Self::Base,
            "SSensorMineLaserTrigger" => Self::Laser {
                value: Box::new(LaserTrigger::read(source, notes)?),
            },
            "SSensorMineProximityTrigger" => Self::Proximity {
                value: Box::new(ProximityTrigger::read(source, notes)?),
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LaserTrigger {
    laser_length: SourcedValue<ResourceFloat32>,
    laser_hit_offset: SourcedValue<ResourceFloat32>,
    mine_front: SourcedValue<DamageVector>,
    laser_offset: SourcedValue<DamageVector>,
}

impl LaserTrigger {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            laser_length: source.single("LaserLength", notes)?,
            laser_hit_offset: source.single("LaserHitOffset", notes)?,
            mine_front: source.inline_class("MineFront", &["Vec3"], notes, DamageVector::read)?,
            laser_offset: source.inline_class(
                "LaserOffset",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ProximityTrigger {
    max_rays_per_mine: SourcedValue<i32>,
    radius: SourcedValue<ResourceFloat32>,
    warning_radius: SourcedValue<ResourceFloat32>,
    angle_vertical: SourcedValue<ResourceFloat32>,
    angle_horizontal: SourcedValue<ResourceFloat32>,
    guide_laser_length: SourcedValue<ResourceFloat32>,
    front: SourcedValue<DamageVector>,
    up: SourcedValue<DamageVector>,
    center: SourcedValue<DamageVector>,
}

impl ProximityTrigger {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            max_rays_per_mine: source.int32("MaxRaysPerMine", notes)?,
            radius: source.single("Radius", notes)?,
            warning_radius: source.single("WarningRadius", notes)?,
            angle_vertical: source.single("AngleVertical", notes)?,
            angle_horizontal: source.single("AngleHorizontal", notes)?,
            guide_laser_length: source.single("GuideLaserLength", notes)?,
            front: source.inline_class("Front", &["Vec3"], notes, DamageVector::read)?,
            up: source.inline_class("Up", &["Vec3"], notes, DamageVector::read)?,
            center: source.inline_class("Center", &["Vec3"], notes, DamageVector::read)?,
        })
    }
}
