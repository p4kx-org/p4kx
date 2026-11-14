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

/// Original penetration distance and radii.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoPenetration {
    base_penetration_distance: SourcedValue<ResourceFloat32>,
    near_radius: SourcedValue<ResourceFloat32>,
    far_radius: SourcedValue<ResourceFloat32>,
}

impl AmmoPenetration {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            base_penetration_distance: source.single("basePenetrationDistance", notes)?,
            near_radius: source.single("nearRadius", notes)?,
            far_radius: source.single("farRadius", notes)?,
        })
    }
}
