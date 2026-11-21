// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_damage::{AuthoredDamage, DAMAGE_TYPES};
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

use super::beam::ProjectileBeamEffect;
use super::penetration::AmmoPenetration;

/// Authored Tachyon damage, ranges and hit parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TachyonProjectile<'source> {
    hit_type: SourcedValue<&'source str>,
    hit_effect: SourcedValue<&'source str>,
    damage: SourcedValue<AuthoredDamage<'source>>,
    full_damage_range: SourcedValue<ResourceFloat32>,
    zero_damage_range: SourcedValue<ResourceFloat32>,
    penetration_params: SourcedValue<AmmoPenetration>,
    beam_effects: SourcedValue<Vec<SourcedValue<ProjectileBeamEffect<'source>>>>,
}

impl<'source> TachyonProjectile<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            hit_type: source.borrowed_string("hitType", notes)?,
            hit_effect: source.borrowed_string("hitEffect", notes)?,
            damage: source.owned("damage", DAMAGE_TYPES, notes, AuthoredDamage::read)?,
            full_damage_range: source.single("fullDamageRange", notes)?,
            zero_damage_range: source.single("zeroDamageRange", notes)?,
            penetration_params: source.inline_class(
                "penetrationParams",
                &["AmmoPenetrationParams"],
                notes,
                AmmoPenetration::read,
            )?,
            beam_effects: source.class_array(
                "beamEffects",
                &["SBeamEffectParams"],
                notes,
                ProjectileBeamEffect::read,
            )?,
        })
    }
}
