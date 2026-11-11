// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_damage::DamageVector;
use super::authored_fields::Observed;
use super::projectile_range::ProjectileRange;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::PublicNameRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[path = "ammo_light.rs"]
mod light;

use light::AmmoLight;

/// Original localized short name, transform and light configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoPresentation<'source> {
    display_name: SourcedValue<PublicNameRef>,
    geometry_transform_params: SourcedValue<AmmoGeometryTransform>,
    light_pool_params: SourcedValue<AmmoLight<'source>>,
}

impl<'source> AmmoPresentation<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            display_name: source.public_name("displayName", projection, notes)?,
            geometry_transform_params: source.strong_pointer(
                "geometryTransformParams",
                &["GeometryTransformParams"],
                notes,
                AmmoGeometryTransform::read,
            )?,
            light_pool_params: source.strong_pointer(
                "lightPoolParams",
                &["PooledLightData"],
                notes,
                AmmoLight::read,
            )?,
        })
    }
}

/// Original angle, rotation and scale words in the geometry transform.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoGeometryTransform {
    init_angles: SourcedValue<DamageVector>,
    random_angles: SourcedValue<DamageVector>,
    rotation_rate: SourcedValue<DamageVector>,
    random_rotation: SourcedValue<DamageVector>,
    random_scale: SourcedValue<ProjectileRange>,
}

impl AmmoGeometryTransform {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            init_angles: source.inline_class("initAngles", &["Vec3"], notes, DamageVector::read)?,
            random_angles: source.inline_class(
                "randomAngles",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
            rotation_rate: source.inline_class(
                "rotationRate",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
            random_rotation: source.inline_class(
                "randomRotation",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
            random_scale: source.inline_class(
                "randomScale",
                &["Range"],
                notes,
                ProjectileRange::read,
            )?,
        })
    }
}
