// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::definition_reference::DefinitionReference;
use crate::owned_reference::OwnedReference;
use crate::owned_reference_read;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldChecks};
use crate::spawn_read::Reader;

/// An authored `TagList`, retaining nullable target identities in source order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct SourceTagList {
    /// The raw tag References, null for an absent array.
    pub(crate) tags: Option<Vec<Option<DefinitionReference>>>,
}

pub(crate) fn check_context<'source>(
    source: &StructValue<'source>,
    path: &str,
    known: &[&str],
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    FieldChecks::for_context(source, path, capture)?.check(known)
}

pub(crate) fn check_observed<'source>(
    source: &StructValue<'source>,
    path: &str,
    known: &[&str],
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    check_context(source, path, known, Some(capture))
}

pub(crate) fn tags_context<'source>(
    source: Option<&StructValue<'source>>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<SourceTagList>, ReadError> {
    source
        .map(|source| {
            let fields = FieldChecks::for_context(source, path, capture)?;
            fields.check(&["tags"])?;
            Ok(SourceTagList {
                tags: fields.references("tags")?,
            })
        })
        .transpose()
}

pub(crate) fn tags_shape_observed<'source>(
    source: Option<&StructValue<'source>>,
    path: &str,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    tag_shape(source, path, Some(capture))
}

pub(crate) fn tag_shape<'source>(
    source: Option<&StructValue<'source>>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    if let Some(source) = source {
        let fields = FieldChecks::for_context(source, path, capture)?;
        fields.check(&["tags"])?;
        fields.reference_shapes("tags")?;
    }
    Ok(())
}
pub(crate) fn list<'source, T>(
    source: &StructValue<'source>,
    name: &str,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
    mut read: impl FnMut(&StructValue<'source>, &str) -> Result<T, ReadError>,
) -> Result<Option<Vec<Option<T>>>, ReadError> {
    let mut found = Vec::new();
    let present = visit_context(source, name, path, capture, |entry, location| {
        found.push(entry.map(|entry| read(entry, location)).transpose()?);
        Ok(())
    })?;
    Ok(present.then_some(found))
}

pub(crate) fn visit_observed<'source>(
    source: &StructValue<'source>,
    name: &str,
    path: &str,
    capture: &dyn FieldCapture<'source>,
    read: impl FnMut(Option<&StructValue<'source>>, &str) -> Result<(), ReadError>,
) -> Result<bool, ReadError> {
    visit_context(source, name, path, Some(capture), read)
}

pub(crate) fn visit_context<'source>(
    source: &StructValue<'source>,
    name: &str,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
    mut read: impl FnMut(Option<&StructValue<'source>>, &str) -> Result<(), ReadError>,
) -> Result<bool, ReadError> {
    let fields = FieldChecks::for_context(source, path, capture)?;
    let Some(values) = fields.array(name)? else {
        return Ok(false);
    };
    for (index, value) in values.iter().enumerate() {
        let location = if path.is_empty() {
            format!("{name}[{index}]")
        } else {
            format!("{path}.{name}[{index}]")
        };
        let entry = match value {
            Value::Null => None,
            Value::Struct(value) => Some(value),
            _ => {
                return Err(fields.invalid(
                    &format!("{name}[{index}]"),
                    value,
                    "a nullable owned struct",
                ));
            }
        };
        read(entry, &location)?;
    }
    Ok(true)
}

pub(crate) fn weaks<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    name: &str,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<Vec<Option<OwnedReference>>>, ReadError> {
    let fields = FieldChecks::for_context(source, path, capture)?;
    fields
        .array(name)?
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    owned_reference_read::read(reader, value)
                        .map(|target| target.map(|target| target.reference))
                        .map_err(|()| {
                            fields.invalid(
                                &format!("{name}[{index}]"),
                                value,
                                "a nullable owner-local weak pointer",
                            )
                        })
                })
                .collect()
        })
        .transpose()
}
