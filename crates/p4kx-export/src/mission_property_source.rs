// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::definition_reference::DefinitionReference;
use crate::owned_reference::OwnedReference;
use crate::owned_reference_read;
use crate::record_read::ReadError;
use crate::source_fields::Fields;
use crate::spawn_read::Reader;

/// An authored `TagList`, retaining nullable target identities in source order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct SourceTagList {
    /// The raw tag References, null for an absent array.
    pub(crate) tags: Option<Vec<Option<DefinitionReference>>>,
}

pub(crate) fn check(source: &StructValue<'_>, path: &str, known: &[&str]) -> Result<(), ReadError> {
    let mut notes = Vec::new();
    Fields::new(source, path).check(known, &mut notes);
    if let Some(error) = notes.into_iter().next() {
        return Err(Box::new(error));
    }
    Ok(())
}

pub(crate) fn tags(
    source: Option<&StructValue<'_>>,
    path: &str,
) -> Result<Option<SourceTagList>, ReadError> {
    source
        .map(|source| {
            check(source, path, &["tags"])?;
            Ok(SourceTagList {
                tags: Fields::new(source, path).references("tags")?,
            })
        })
        .transpose()
}

pub(crate) fn list<'f, T>(
    source: &StructValue<'f>,
    name: &str,
    path: &str,
    mut read: impl FnMut(&StructValue<'f>, &str) -> Result<T, ReadError>,
) -> Result<Option<Vec<Option<T>>>, ReadError> {
    let fields = Fields::new(source, path);
    fields
        .array(name)?
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let location = if path.is_empty() {
                        format!("{name}[{index}]")
                    } else {
                        format!("{path}.{name}[{index}]")
                    };
                    match value {
                        Value::Null => Ok(None),
                        Value::Struct(value) => read(value, &location).map(Some),
                        _ => Err(Box::new(fields.invalid(
                            &format!("{name}[{index}]"),
                            value,
                            "a nullable owned struct",
                        )) as ReadError),
                    }
                })
                .collect()
        })
        .transpose()
}

pub(crate) fn weak(
    reader: &Reader<'_, '_>,
    source: &StructValue<'_>,
    name: &str,
    path: &str,
) -> Result<Option<OwnedReference>, ReadError> {
    let Some(value) = source.field(name) else {
        return Ok(None);
    };
    owned_reference_read::read(reader, value)
        .map(|target| target.map(|target| target.reference))
        .map_err(|()| {
            Box::new(Fields::new(source, path).invalid(
                name,
                value,
                "a nullable owner-local weak pointer",
            )) as ReadError
        })
}

pub(crate) fn weaks(
    reader: &Reader<'_, '_>,
    source: &StructValue<'_>,
    name: &str,
    path: &str,
) -> Result<Option<Vec<Option<OwnedReference>>>, ReadError> {
    let fields = Fields::new(source, path);
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
                            Box::new(fields.invalid(
                                &format!("{name}[{index}]"),
                                value,
                                "a nullable owner-local weak pointer",
                            )) as ReadError
                        })
                })
                .collect()
        })
        .transpose()
}
