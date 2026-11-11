// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_damage::DamageVector;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Stored radii, orientation and physical shape fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ExplosionGeometry {
    min_radius: SourcedValue<ResourceFloat32>,
    max_radius: SourcedValue<ResourceFloat32>,
    sound_radius: SourcedValue<ResourceFloat32>,
    min_phys_radius: SourcedValue<ResourceFloat32>,
    max_phys_radius: SourcedValue<ResourceFloat32>,
    angle: SourcedValue<ResourceFloat32>,
    angle_vertical: SourcedValue<ResourceFloat32>,
    explosion_front: SourcedValue<DamageVector>,
    explosion_up: SourcedValue<DamageVector>,
    explosion_zone: SourcedValue<String>,
    pressure: SourcedValue<ResourceFloat32>,
    hole_size: SourcedValue<ResourceFloat32>,
    terrain_hole_size: SourcedValue<ResourceFloat32>,
}

impl ExplosionGeometry {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min_radius: source.single("minRadius", notes)?,
            max_radius: source.single("maxRadius", notes)?,
            sound_radius: source.single("soundRadius", notes)?,
            min_phys_radius: source.single("minPhysRadius", notes)?,
            max_phys_radius: source.single("maxPhysRadius", notes)?,
            angle: source.single("angle", notes)?,
            angle_vertical: source.single("angleVertical", notes)?,
            explosion_front: source.owned(
                "explosionFront",
                &["Vec3"],
                notes,
                DamageVector::read,
            )?,
            explosion_up: source.owned("explosionUp", &["Vec3"], notes, DamageVector::read)?,
            explosion_zone: source.enumeration("explosionZone", notes)?,
            pressure: source.single("pressure", notes)?,
            hole_size: source.single("holeSize", notes)?,
            terrain_hole_size: source.single("terrainHoleSize", notes)?,
        })
    }
}
