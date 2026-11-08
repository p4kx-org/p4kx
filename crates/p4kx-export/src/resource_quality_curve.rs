// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::resource_quality_source::ResourceQualityCurve;
use crate::resource_value_read;
use crate::source_fields::FieldError;

/// Reads the resource-owned distribution without changing numeric representation.
pub(crate) fn read<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityCurve<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    if source.type_name() == "CraftingQualityDistributionNormal" {
        fields.check(&["min", "max", "mean", "stddev"], notes);
    } else {
        fields.check(&["min", "max"], notes);
    }
    let mean = resource_value_read::number(&fields, "mean", notes)?;
    let stddev = resource_value_read::number(&fields, "stddev", notes)?;
    Ok(ResourceQualityCurve {
        kind: fields.source_type()?,
        min: fields.int32("min", notes)?,
        max: fields.int32("max", notes)?,
        mean,
        stddev,
    })
}
