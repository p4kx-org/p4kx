// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue, Value};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::quality_source::{QualityBandSource, QualityQuantizationSource};
use crate::resource_value_read::{self, NumericPresence};
use crate::source_fields::FieldError;

/// Reads every band in its authored position, including null or unsupported members.
pub(crate) fn read<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<QualityQuantizationSource<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    fields.check(&["bands"], notes);
    let bands = fields.array("bands", notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let location = format!("{path}.bands[{index}]");
                if matches!(value, Value::Null) {
                    notes.push(FieldError::Malformed {
                        source_type: source.type_name().to_owned(),
                        field: location.clone(),
                        expected: "a non-null CraftingQualityQuantizationBand Class value",
                    });
                }
                context
                    .structure_element(value, source.type_name(), &location, notes)?
                    .try_map(|entry| band(entry, &location, context, notes))
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
) -> Result<QualityBandSource<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    if source.type_name() != "CraftingQualityQuantizationBand" {
        notes.push(crate::source_fields::Fields::new(source, path).unsupported(""));
        return Ok(QualityBandSource {
            source_type: fields.source_type()?,
            start: context.selector(&format!("{path}.start"), SourceValue::Unsupported)?,
            end: context.selector(&format!("{path}.end"), SourceValue::Unsupported)?,
            mapped_value: context
                .selector(&format!("{path}.mappedValue"), SourceValue::Unsupported)?,
        });
    }
    fields.check(&["start", "end", "mappedValue"], notes);
    if let (Some(Value::Int32(start)), Some(Value::Int32(end))) =
        (source.field("start"), source.field("end"))
        && end < start
    {
        notes.push(FieldError::Malformed {
            source_type: source.type_name().to_owned(),
            field: format!("{path}.end"),
            expected: "an inclusive band end at least its start",
        });
    }
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
