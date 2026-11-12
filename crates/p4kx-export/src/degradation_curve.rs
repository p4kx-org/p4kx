// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The original curve controls and authored point slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DegradationCurve {
    use_lut: SourcedValue<bool>,
    points: SourcedValue<Vec<SourcedValue<CurvePoint>>>,
}

impl DegradationCurve {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            use_lut: source.boolean("useLUT", notes)?,
            points: source.array("points", &["Vec2"], notes, CurvePoint::read)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 2] {
        [
            ("useLUT", self.use_lut.state()),
            ("points", self.points.state()),
        ]
    }
}

/// The exact stored coordinates of an authored curve point.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CurvePoint {
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
}

impl CurvePoint {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            x: source.single("x", notes)?,
            y: source.single("y", notes)?,
        })
    }
}
