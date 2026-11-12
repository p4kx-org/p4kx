// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct InventoryAxes {
    x: SourcedValue<ResourceFloat32>,
    y: SourcedValue<ResourceFloat32>,
}

impl InventoryAxes {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            x: source.single("x", notes)?,
            y: source.single("y", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 2] {
        [("x", self.x.state()), ("y", self.y.state())]
    }
}

/// A physical two-coordinate inventory dimension.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryVector2<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    axes: InventoryAxes,
}

impl<'source> InventoryVector2<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let axes = InventoryAxes::read(source, notes)?;
        let source = UnitSource::read(source, axes.projected())?;
        Ok(Self { source, axes })
    }
}

/// Physical three-coordinate inventory dimensions or offsets.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryVector3<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    axes: InventoryAxes,
    z: SourcedValue<ResourceFloat32>,
}

impl<'source> InventoryVector3<'source> {
    pub(super) fn class_field(
        source: Observed<'_, '_, '_, 'source>,
        field: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        source.declared_class_to(field, "Vec3", notes, Self::read)
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let axes = InventoryAxes::read(source, notes)?;
        let z = source.single("z", notes)?;
        let source = UnitSource::read(
            source,
            axes.projected().into_iter().chain([("z", z.state())]),
        )?;
        Ok(Self { source, axes, z })
    }
}
