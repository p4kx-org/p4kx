// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The original three color channels.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "ProjectileTint", transform = every_field_required)]
pub(crate) struct AuthoredRgb {
    r: SourcedValue<ResourceFloat32>,
    g: SourcedValue<ResourceFloat32>,
    b: SourcedValue<ResourceFloat32>,
}

impl AuthoredRgb {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            r: source.single("r", notes)?,
            g: source.single("g", notes)?,
            b: source.single("b", notes)?,
        })
    }
}
