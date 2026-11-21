// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const MINIMUM: &str = "minimum";
const MAXIMUM: &str = "maximum";

/// The two authored Single endpoints of a Range.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileRange {
    minimum: SourcedValue<ResourceFloat32>,
    maximum: SourcedValue<ResourceFloat32>,
}

impl ProjectileRange {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            minimum: source.single(MINIMUM, notes)?,
            maximum: source.single(MAXIMUM, notes)?,
        })
    }

    pub(super) const fn minimum(&self) -> &SourcedValue<ResourceFloat32> {
        &self.minimum
    }

    pub(super) const fn maximum(&self) -> &SourcedValue<ResourceFloat32> {
        &self.maximum
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 2] {
        [
            (MINIMUM, self.minimum.state()),
            (MAXIMUM, self.maximum.state()),
        ]
    }
}
