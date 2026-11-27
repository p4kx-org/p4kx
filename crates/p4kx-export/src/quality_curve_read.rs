// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::quality_curve::QualityCurve;
use crate::source_fields::{FieldError, Fields};

/// Reads the shared exact quality payload and reports fields beyond its supported declaration.
pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<QualityCurve<'source>, FieldError> {
    let fields = Fields::new(source, path);
    let (min, max, mean, stddev) = match source.type_name() {
        "CraftingQualityDistributionNormal" => {
            fields.check(&["min", "max", "mean", "stddev"], notes);
            (
                fields.int32("min")?,
                fields.int32("max")?,
                fields.number("mean")?,
                fields.number("stddev")?,
            )
        }
        "CraftingQualityDistributionUniform" => {
            fields.check(&["min", "max"], notes);
            (fields.int32("min")?, fields.int32("max")?, None, None)
        }
        "CraftingQualityDistribution_Base" | "CraftingQualityDistribution_Base_NonRef" => {
            fields.check(&[], notes);
            (None, None, None, None)
        }
        _ => return Err(fields.unsupported("")),
    };
    Ok(QualityCurve {
        kind: source.type_name(),
        min,
        max,
        mean,
        stddev,
    })
}
