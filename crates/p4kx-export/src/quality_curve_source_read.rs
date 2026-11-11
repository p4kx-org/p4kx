// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::quality_source::QualityCurveSource;
use crate::resource_value_read::{self, NumericPresence};
use crate::source_fields::{FieldCapture, FieldError};

/// Reads an inline distribution without changing numeric representation.
pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'source>,
) -> Result<QualityCurveSource, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    if source.type_name() == "CraftingQualityDistributionNormal" {
        fields.check_observed(&["min", "max", "mean", "stddev"], notes)?;
    } else {
        fields.check_observed(&["min", "max"], notes)?;
    }
    let mean = resource_value_read::number(&fields, "mean", notes)?;
    let stddev = resource_value_read::number(&fields, "stddev", notes)?;
    Ok(QualityCurveSource {
        min: resource_value_read::int32_number(&fields, "min", NumericPresence::Optional, notes)?,
        max: resource_value_read::int32_number(&fields, "max", NumericPresence::Optional, notes)?,
        mean,
        stddev,
    })
}
