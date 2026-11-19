// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::InventoryVector2;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Ordered knots and the original lookup table setting of an authored curve.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct OrderedCurve<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(rename = "useLUT")]
    use_lut: SourcedValue<bool>,
    points: SourcedValue<Vec<SourcedValue<InventoryVector2<'source>>>>,
}

impl<'source> OrderedCurve<'source> {
    /// Borrows the curve type and remaining original fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original lookup table setting.
    #[must_use]
    pub const fn use_lut(&self) -> &SourcedValue<bool> {
        &self.use_lut
    }

    /// Borrows ordered original point slots.
    #[must_use]
    pub const fn points(&self) -> &SourcedValue<Vec<SourcedValue<InventoryVector2<'source>>>> {
        &self.points
    }

    pub(in crate::catalog_v3) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let use_lut = fields.boolean("useLUT", notes)?;
        let points = fields.declared_class_array("points", notes, InventoryVector2::read)?;
        Self::with_points(fields, use_lut, points)
    }

    pub(in crate::catalog_v3) fn read_bezier(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let use_lut = fields.boolean("useLUT", notes)?;
        let points = fields.class_array_to("points", "Vec2", notes, InventoryVector2::read)?;
        Self::with_points(fields, use_lut, points)
    }

    fn with_points(
        fields: Observed<'_, '_, '_, 'source>,
        use_lut: SourcedValue<bool>,
        points: SourcedValue<Vec<SourcedValue<InventoryVector2<'source>>>>,
    ) -> Result<Self, CatalogError> {
        let point_state = match points.value() {
            Some(points)
                if points.iter().any(|point| {
                    !matches!(
                        point.state(),
                        SourceValueState::Present | SourceValueState::Null
                    )
                }) =>
            {
                SourceValueState::Invalid
            }
            _ => points.state(),
        };
        let source = UnitSource::read(
            fields,
            [("useLUT", use_lut.state()), ("points", point_state)],
        )?;
        Ok(Self {
            source,
            use_lut,
            points,
        })
    }
}
