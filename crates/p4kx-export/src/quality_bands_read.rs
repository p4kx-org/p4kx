// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue, Value};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::quality_source::{QualityBandSource, QualityQuantizationSource};
use crate::resource_value_read::{self, NumericPresence};
use crate::source_fields::{FieldCapture, FieldError};

/// Reads every band in its authored position, including null or unsupported members.
pub(crate) fn read<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualityQuantizationSource<'f>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    fields.check_observed(&["bands"], notes)?;
    let bands = fields.array("bands", notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let location = format!("{path}.bands[{index}]");
                if matches!(value, Value::Null) {
                    notes.push(fields.invalid(
                        &format!("bands[{index}]"),
                        value,
                        "a non-null CraftingQualityQuantizationBand Class value",
                    )?);
                }
                fields
                    .structure_element(&format!("bands[{index}]"), value, notes)?
                    .try_map(|entry| band(entry, &location, context, notes, capture))
            })
            .collect::<Result<_, CatalogError>>()
    })?;
    Ok(QualityQuantizationSource { bands })
}

fn band<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualityBandSource<'f>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    if source.type_name() != "CraftingQualityQuantizationBand" {
        fields.note_unsupported(notes)?;
        return Ok(QualityBandSource {
            source_type: fields.source_type()?,
            start: context.selector(&format!("{path}.start"), SourceValue::Unsupported)?,
            end: context.selector(&format!("{path}.end"), SourceValue::Unsupported)?,
            mapped_value: context
                .selector(&format!("{path}.mappedValue"), SourceValue::Unsupported)?,
        });
    }
    fields.check_observed(&["start", "end", "mappedValue"], notes)?;
    validate_interval(&fields, notes)?;
    Ok(QualityBandSource {
        source_type: fields.source_type()?,
        start: resource_value_read::int32_number(
            &fields,
            "start",
            NumericPresence::Required,
            notes,
        )?,
        end: resource_value_read::int32_number(&fields, "end", NumericPresence::Required, notes)?,
        mapped_value: resource_value_read::int32_number(
            &fields,
            "mappedValue",
            NumericPresence::Required,
            notes,
        )?,
    })
}

fn validate_interval(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<(), CatalogError> {
    if let (Some(Value::Int32(start)), Some(end @ Value::Int32(end_value))) =
        (fields.source().field("start"), fields.source().field("end"))
        && end_value < start
    {
        notes.push(fields.invalid("end", end, "an inclusive band end at least its start")?);
    }
    Ok(())
}
