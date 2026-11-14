// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The stored shake settings and their original offset declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MissileShake<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    offset_angle: SourcedValue<MissileAxes<'source>>,
    offset_position: SourcedValue<MissileAxes<'source>>,
    randomness: SourcedValue<ResourceFloat32>,
    range: SourcedValue<ResourceFloat32>,
    duration: SourcedValue<ResourceFloat32>,
    frequency: SourcedValue<ResourceFloat32>,
}

impl<'source> MissileShake<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let offset_angle =
            source.declared_class_to("offsetAngle", "Ang3", notes, MissileAxes::read)?;
        let offset_position =
            source.declared_class_to("offsetPosition", "Vec3", notes, MissileAxes::read)?;
        let randomness = source.single("randomness", notes)?;
        let range = source.single("range", notes)?;
        let duration = source.single("duration", notes)?;
        let frequency = source.single("frequency", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("offsetAngle", offset_angle.state()),
                ("offsetPosition", offset_position.state()),
                ("randomness", randomness.state()),
                ("range", range.state()),
                ("duration", duration.state()),
                ("frequency", frequency.state()),
            ],
        )?;
        Ok(Self {
            source,
            offset_angle,
            offset_position,
            randomness,
            range,
            duration,
            frequency,
        })
    }
}

/// Three original axis components in the source declaration's own units.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MissileAxes<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
    z: SourcedValue<ResourceFloat32>,
}

impl<'source> MissileAxes<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let x = source.single("x", notes)?;
        let y = source.single("y", notes)?;
        let z = source.single("z", notes)?;
        let source = UnitSource::read(
            source,
            [("x", x.state()), ("y", y.state()), ("z", z.state())],
        )?;
        Ok(Self { source, x, y, z })
    }
}
