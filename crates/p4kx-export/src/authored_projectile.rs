// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::projectile_range as range;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

#[path = "projectile_beam.rs"]
mod beam;
#[path = "projectile_bullet.rs"]
mod bullet;
#[path = "projectile_countermeasure.rs"]
mod countermeasure;
#[path = "projectile_detonation.rs"]
mod detonation;
#[path = "projectile_penetration.rs"]
mod penetration;
#[path = "projectile_tachyon.rs"]
mod tachyon;
#[path = "projectile_visual.rs"]
mod visual;

use bullet::BulletProjectile;
use countermeasure::CountermeasureProjectile;
use detonation::{ProjectileDetonation, ProjectileProximity};
use tachyon::TachyonProjectile;

const BASE: &str = "ProjectileParams";
const BULLET: &str = "BulletProjectileParams";
const TACHYON: &str = "TachyonProjectileParams";
const COUNTERMEASURE: &str = "CounterMeasureProjectileParams";
const TYPES: &[&str] = &[BASE, BULLET, TACHYON, COUNTERMEASURE];

/// One original projectile occurrence with inherited configuration and a concrete branch.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AuthoredProjectile<'source> {
    detonation_params: SourcedValue<Box<ProjectileDetonation<'source>>>,
    proximity_trigger_params: SourcedValue<ProjectileProximity>,
    kind: ProjectileKind<'source>,
}

impl<'source> AuthoredProjectile<'source> {
    pub(super) fn read_ammo(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        source.strong_pointer("projectileParams", TYPES, notes, Self::read)
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            detonation_params: source.owned(
                "detonationParams",
                &["ProjectileDetonationParams"],
                notes,
                |source, notes| ProjectileDetonation::read(source, notes).map(Box::new),
            )?,
            proximity_trigger_params: source.owned(
                "proximityTriggerParams",
                &["ProjectileProximityTriggerParams"],
                notes,
                ProjectileProximity::read,
            )?,
            kind: ProjectileKind::read(source, notes)?,
        })
    }
}

/// Concrete behavior of the original projectile occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum ProjectileKind<'source> {
    Base,
    Bullet {
        value: Box<BulletProjectile<'source>>,
    },
    Tachyon {
        value: Box<TachyonProjectile<'source>>,
    },
    Countermeasure {
        value: Box<CountermeasureProjectile>,
    },
}

impl<'source> ProjectileKind<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            BASE => Self::Base,
            BULLET => Self::Bullet {
                value: Box::new(BulletProjectile::read(source, notes)?),
            },
            TACHYON => Self::Tachyon {
                value: Box::new(TachyonProjectile::read(source, notes)?),
            },
            COUNTERMEASURE => Self::Countermeasure {
                value: Box::new(CountermeasureProjectile::read(source, notes)?),
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

#[cfg(test)]
#[path = "authored_projectile_tests.rs"]
mod tests;
