// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_damage::{AuthoredDamage, DAMAGE_TYPES, DamageVector};
use super::authored_fields::Observed;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;
use crate::{CatalogError, ExportAccountingError};

#[path = "explosion_effects.rs"]
mod effects;
#[path = "explosion_geometry.rs"]
mod geometry;

use effects::{ExplosionEffects, Flashbang, Fragments, Shockwave};
use geometry::ExplosionGeometry;

pub(super) const EXPLOSION_TYPES: &[&str] = &[
    "ExplosionBaseParams",
    "ExplosionParams",
    "ExplosionByRecordParams",
];

/// The original alternatives of an owned `ExplosionBaseParams` occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum AuthoredExplosion<'source> {
    Base,
    Inline {
        value: Box<ExplosionParameters<'source>>,
    },
    ByRecord {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        record: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
}

impl<'source> AuthoredExplosion<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "ExplosionBaseParams" => Self::Base,
            "ExplosionParams" => Self::Inline {
                value: Box::new(ExplosionParameters::read(source, notes)?),
            },
            "ExplosionByRecordParams" => Self::ByRecord {
                record: source.record_link("record", notes)?,
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Original configuration fields of one actual `ExplosionParams` occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ExplosionParameters<'source> {
    friendly_fire: SourcedValue<String>,
    #[serde(flatten)]
    geometry: ExplosionGeometry,
    #[serde(flatten)]
    effects: ExplosionEffects,
    damage: SourcedValue<AuthoredDamage<'source>>,
    flashbang_params: SourcedValue<Flashbang>,
    shockwave_params: SourcedValue<Shockwave>,
    fragment_params: SourcedValue<Fragments>,
    hit_type: SourcedValue<String>,
    offset: SourcedValue<DamageVector>,
    direction: SourcedValue<DamageVector>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    radar_contact_type: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> ExplosionParameters<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            friendly_fire: source.enumeration("friendlyFire", notes)?,
            geometry: ExplosionGeometry::read(source, notes)?,
            effects: ExplosionEffects::read(source, notes)?,
            damage: source.owned("damage", DAMAGE_TYPES, notes, AuthoredDamage::read)?,
            flashbang_params: source.owned(
                "flashbangParams",
                &["ExplosionFlashbangParams"],
                notes,
                Flashbang::read,
            )?,
            shockwave_params: source.owned(
                "shockwaveParams",
                &["ShockwaveParams"],
                notes,
                Shockwave::read,
            )?,
            fragment_params: source.owned(
                "fragmentParams",
                &["ExplosiveFragmentParams"],
                notes,
                Fragments::read,
            )?,
            hit_type: source.string("hitType", notes)?,
            offset: source.owned("Offset", &["Vec3"], notes, DamageVector::read)?,
            direction: source.owned("Direction", &["Vec3"], notes, DamageVector::read)?,
            radar_contact_type: source.record_link("radarContactType", notes)?,
        })
    }
}
