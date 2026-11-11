// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::observed::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The stored fields of one distortion component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DistortionFields {
    decay_delay: SourcedValue<ResourceFloat32>,
    decay_rate: SourcedValue<ResourceFloat32>,
    maximum: SourcedValue<ResourceFloat32>,
    warning_ratio: SourcedValue<ResourceFloat32>,
    recovery_ratio: SourcedValue<ResourceFloat32>,
    power_ratio_at_max_distortion: SourcedValue<ResourceFloat32>,
    power_change_only_at_max_distortion: SourcedValue<bool>,
}

impl DistortionFields {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            decay_delay: source.single("DecayDelay", notes)?,
            decay_rate: source.single("DecayRate", notes)?,
            maximum: source.single("Maximum", notes)?,
            warning_ratio: source.single("WarningRatio", notes)?,
            recovery_ratio: source.single("RecoveryRatio", notes)?,
            power_ratio_at_max_distortion: source.single("PowerRatioAtMaxDistortion", notes)?,
            power_change_only_at_max_distortion: source
                .boolean("PowerChangeOnlyAtMaxDistortion", notes)?,
        })
    }
}
