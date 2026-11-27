// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "QuatT";

/// The authored position and rotation components of a local grip offset.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CarryableGripOffset<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    rotation: SourcedValue<InventoryVector3<'source>>,
    position: SourcedValue<InventoryVector3<'source>>,
}

impl<'source> CarryableGripOffset<'source> {
    /// Borrows the concrete offset declaration and its unprojected members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original Ang3 rotation components.
    #[must_use]
    pub const fn rotation(&self) -> &SourcedValue<InventoryVector3<'source>> {
        &self.rotation
    }

    /// Borrows the original Vec3 position components.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<InventoryVector3<'source>> {
        &self.position
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let rotation =
            source.declared_class_to("Rotation", "Ang3", notes, InventoryVector3::read)?;
        let position = InventoryVector3::class_field(source, "Position", notes)?;
        Ok(Self {
            source: UnitSource::read(
                source,
                [
                    ("Rotation", rotation.state()),
                    ("Position", position.state()),
                ],
            )?,
            rotation,
            position,
        })
    }
}
