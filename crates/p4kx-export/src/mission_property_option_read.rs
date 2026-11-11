// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_option::{FloatOption, IntegerOption, StringOption, TagOption};
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::{self, FieldCapture, FieldChecks};
use crate::spawn_read::Reader;
use crate::template_property_kind::{
    FLOAT_OPTION_TYPE, INTEGER_OPTION_TYPE, STRING_OPTION_TYPE, TAGS_OPTION_TYPE,
};
use crate::texts::Texts;

#[path = "mission_property_option_metadata.rs"]
mod option_metadata;

use self::option_metadata::{metadata, validate_metadata};

const BASE_FIELDS: [&str; 4] = [
    "textId",
    "weighting",
    "DEBUG_forceChooseThisOption",
    "dependentProperties",
];

fn check<'source>(
    source: &StructValue<'source>,
    expected: &str,
    fields: &[&str],
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    if source.type_name() != expected {
        return Err(source_fields::unknown_struct(source, "options", capture));
    }
    let checks = FieldChecks::for_context(source, "options", capture)?;
    for field in source.fields() {
        if !BASE_FIELDS.contains(&field.name()) && !fields.contains(&field.name()) {
            return Err(checks.unsupported(field));
        }
    }
    Ok(())
}

fn check_string<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check(source, STRING_OPTION_TYPE, &["value"], capture)
}
fn check_integer<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check(
        source,
        INTEGER_OPTION_TYPE,
        &["value", "variation"],
        capture,
    )
}
fn check_float<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check(source, FLOAT_OPTION_TYPE, &["value", "variation"], capture)
}
fn check_tags<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check(source, TAGS_OPTION_TYPE, &["tags", "negativeTags"], capture)
}

fn string_value<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<&'source str>, ReadError> {
    FieldChecks::for_context(source, path, capture)?.text("value")
}

struct NumericValues<T> {
    value: Option<T>,
    variation: Option<T>,
}

fn integer_values<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<NumericValues<i32>, ReadError> {
    let fields = FieldChecks::for_context(source, path, capture)?;
    Ok(NumericValues {
        value: fields.int32("value")?,
        variation: fields.int32("variation")?,
    })
}
fn float_values<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<NumericValues<f32>, ReadError> {
    let fields = FieldChecks::for_context(source, path, capture)?;
    Ok(NumericValues {
        value: fields.number("value")?,
        variation: fields.number("variation")?,
    })
}

pub(crate) fn string<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<StringOption<'source>, ReadError> {
    check_string(source, capture)?;
    Ok(StringOption {
        metadata: metadata(reader, source, path, texts, capture)?,
        value: string_value(source, path, capture)?.map(str::to_owned),
    })
}

pub(crate) fn integer<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<IntegerOption<'source>, ReadError> {
    check_integer(source, capture)?;
    let metadata = metadata(reader, source, path, texts, capture)?;
    let values = integer_values(source, path, capture)?;
    Ok(IntegerOption {
        metadata,
        value: values.value,
        variation: values.variation,
    })
}

pub(crate) fn float<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<FloatOption<'source>, ReadError> {
    check_float(source, capture)?;
    let metadata = metadata(reader, source, path, texts, capture)?;
    let values = float_values(source, path, capture)?;
    Ok(FloatOption {
        metadata,
        value: values.value,
        variation: values.variation,
    })
}

pub(crate) fn tags<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<TagOption<'source>, ReadError> {
    check_tags(source, capture)?;
    let fields = FieldChecks::for_context(source, path, capture)?;
    Ok(TagOption {
        metadata: metadata(reader, source, path, texts, capture)?,
        tags: reader.tag_list(Some(source), "tags"),
        negative_tags: reader.tag_list(Some(source), "negativeTags"),
        source_tags: mission_property_source::tags_context(
            fields.structure("tags")?,
            &format!("{path}.tags"),
            capture,
        )?,
        source_negative_tags: mission_property_source::tags_context(
            fields.structure("negativeTags")?,
            &format!("{path}.negativeTags"),
            capture,
        )?,
    })
}

/// Checks the same source while retaining actual original guard failures.
pub(crate) fn validate_string_observed<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    string_context(source, path, Some(capture))
}

fn string_context<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check_string(source, capture)?;
    validate_metadata(source, path, capture)?;
    string_value(source, path, capture)?;
    Ok(())
}

/// Checks an integer option with the original Int32 requirements.
#[cfg(test)]
pub(crate) fn validate_integer(source: &StructValue<'_>, path: &str) -> Result<(), ReadError> {
    integer_context(source, path, None)
}

/// Checks the same source while retaining actual original guard failures.
pub(crate) fn validate_integer_observed<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    integer_context(source, path, Some(capture))
}

fn integer_context<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check_integer(source, capture)?;
    validate_metadata(source, path, capture)?;
    integer_values(source, path, capture)?;
    Ok(())
}

/// Checks the same source while retaining actual original guard failures.
pub(crate) fn validate_float_observed<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    float_context(source, path, Some(capture))
}

fn float_context<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check_float(source, capture)?;
    validate_metadata(source, path, capture)?;
    float_values(source, path, capture)?;
    Ok(())
}

/// Checks the same source while retaining actual original guard failures.
pub(crate) fn validate_tags_observed<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    tags_context(source, path, Some(capture))
}

fn tags_context<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check_tags(source, capture)?;
    validate_metadata(source, path, capture)?;
    let fields = FieldChecks::for_context(source, path, capture)?;
    mission_property_source::tag_shape(
        fields.structure("tags")?,
        &format!("{path}.tags"),
        capture,
    )?;
    mission_property_source::tag_shape(
        fields.structure("negativeTags")?,
        &format!("{path}.negativeTags"),
        capture,
    )?;
    Ok(())
}
