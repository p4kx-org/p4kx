// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::ExportAccountingError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[path = "ammo_particle_physics.rs"]
mod particle;

use particle::AmmoParticlePhysics;

const BASE: &str = "SEntityBasePhysicsControllerParams";
const PARTICLE: &str = "SEntityParticlePhysicsControllerParams";

/// The original physics controller wrapper and its owned `PhysType`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoPhysics<'source> {
    phys_type: SourcedValue<AmmoPhysicsType<'source>>,
}

impl<'source> AmmoPhysics<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            phys_type: source.owned("PhysType", &[BASE, PARTICLE], notes, AmmoPhysicsType::read)?,
        })
    }
}

/// Shared physics fields and the concrete controller branch.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoPhysicsType<'source> {
    #[serde(flatten)]
    common: AmmoPhysicsBase,
    kind: AmmoPhysicsKind<'source>,
}

impl<'source> AmmoPhysicsType<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let common = AmmoPhysicsBase::read(source, notes)?;
        let kind = match source.source_type()? {
            BASE => AmmoPhysicsKind::Base,
            PARTICLE => AmmoPhysicsKind::Particle {
                value: Box::new(AmmoParticlePhysics::read(source, notes)?),
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(Self { common, kind })
    }
}

/// The actual recognized physics controller subtype.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum AmmoPhysicsKind<'source> {
    Base,
    Particle {
        value: Box<AmmoParticlePhysics<'source>>,
    },
}

/// Original fields inherited by supported physics controllers.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoPhysicsBase {
    mass: SourcedValue<ResourceFloat32>,
    compounding_allowed: SourcedValue<bool>,
    breakable_params: SourcedValue<AmmoBreakablePhysics>,
    game_collision_class: SourcedValue<AmmoCollisionClass>,
    spawn_box_scale: SourcedValue<ResourceFloat32>,
}

impl AmmoPhysicsBase {
    pub(in crate::catalog_v3) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            mass: source.single("Mass", notes)?,
            compounding_allowed: source.boolean("compoundingAllowed", notes)?,
            breakable_params: source.owned(
                "breakableParams",
                &["SBreakablePhysicsParams"],
                notes,
                AmmoBreakablePhysics::read,
            )?,
            game_collision_class: source.owned(
                "gameCollisionClass",
                &["SGameCollisionClass"],
                notes,
                AmmoCollisionClass::read,
            )?,
            spawn_box_scale: source.single("spawnBoxScale", notes)?,
        })
    }
}

/// Original breakable physics parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoBreakablePhysics {
    max_simultaneous_cracks: SourcedValue<i32>,
    max_push_force: SourcedValue<ResourceFloat32>,
    max_pull_force: SourcedValue<ResourceFloat32>,
    max_shift_force: SourcedValue<ResourceFloat32>,
    max_twist_torque: SourcedValue<ResourceFloat32>,
    max_bend_torque: SourcedValue<ResourceFloat32>,
    crack_weaken: SourcedValue<ResourceFloat32>,
}

impl AmmoBreakablePhysics {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            max_simultaneous_cracks: source.int32("maxSimultaneousCracks", notes)?,
            max_push_force: source.single("maxPushForce", notes)?,
            max_pull_force: source.single("maxPullForce", notes)?,
            max_shift_force: source.single("maxShiftForce", notes)?,
            max_twist_torque: source.single("maxTwistTorque", notes)?,
            max_bend_torque: source.single("maxBendTorque", notes)?,
            crack_weaken: source.single("crackWeaken", notes)?,
        })
    }
}

/// Original collision classification and ignored class.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoCollisionClass {
    game_collision_class: SourcedValue<String>,
    game_ignore_collision_class: SourcedValue<String>,
}

impl AmmoCollisionClass {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            game_collision_class: source.enumeration("gameCollisionClass", notes)?,
            game_ignore_collision_class: source.enumeration("gameIgnoreCollisionClass", notes)?,
        })
    }
}
