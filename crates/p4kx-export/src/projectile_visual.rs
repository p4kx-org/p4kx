// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Original Bullet rendering and hit-effect inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileVisual<'source> {
    material: SourcedValue<ProjectileMaterial<'source>>,
    max_length: SourcedValue<ResourceFloat32>,
    mesh_offset: SourcedValue<ResourceFloat32>,
    geometry_radius: SourcedValue<ResourceFloat32>,
    render_frequency: SourcedValue<i32>,
    render_probability: SourcedValue<ResourceFloat32>,
    hit_effect: SourcedValue<&'source str>,
}

impl<'source> ProjectileVisual<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            material: source.inline_class(
                "Material",
                &["GlobalResourceMaterial"],
                notes,
                ProjectileMaterial::read,
            )?,
            max_length: source.single("maxLength", notes)?,
            mesh_offset: source.single("meshOffset", notes)?,
            geometry_radius: source.single("geometryRadius", notes)?,
            render_frequency: source.int32("renderFrequency", notes)?,
            render_probability: source.single("renderProbability", notes)?,
            hit_effect: source.borrowed_string("hitEffect", notes)?,
        })
    }
}

/// Original material resource path.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileMaterial<'source> {
    path: SourcedValue<&'source str>,
}

impl<'source> ProjectileMaterial<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            path: source.borrowed_string("path", notes)?,
        })
    }
}
