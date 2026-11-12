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
use crate::source_value_catalog::SourceArenaOutput;

/// The three original power-range slots of a resource state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourcePowerRanges<'source> {
    source_type: SourcedValue<&'source str>,
    low: SourcedValue<PowerRange<'source>>,
    medium: SourcedValue<PowerRange<'source>>,
    high: SourcedValue<PowerRange<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourcePowerRanges<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let low = source.declared_class("low", notes, PowerRange::read)?;
        let medium = source.declared_class("medium", notes, PowerRange::read)?;
        let high = source.declared_class("high", notes, PowerRange::read)?;
        let additional_fields = source.authored_members_except([
            ("low", low.state()),
            ("medium", medium.state()),
            ("high", high.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            low,
            medium,
            high,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PowerRange<'source> {
    source_type: SourcedValue<&'source str>,
    start: SourcedValue<i32>,
    modifier: SourcedValue<ResourceFloat32>,
    register_range: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> PowerRange<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let start = source.int32("start", notes)?;
        let modifier = source.single("modifier", notes)?;
        let register_range = source.boolean("registerRange", notes)?;
        let additional_fields = source.authored_members_except([
            ("start", start.state()),
            ("modifier", modifier.state()),
            ("registerRange", register_range.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            start,
            modifier,
            register_range,
            additional_fields,
        })
    }
}
