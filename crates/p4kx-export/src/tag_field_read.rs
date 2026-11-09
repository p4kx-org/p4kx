// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceValue, SourcedValue, Value, ValueError};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

pub(super) fn legacy_guid(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<u32>, CatalogError> {
    fields.read("legacyGUID", "a nullable UInt32", notes, |value| {
        if let Value::UInt32(value) = value {
            Some(*value)
        } else {
            None
        }
    })
}

pub(super) fn references(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
    cancel: Option<&CancelToken>,
) -> Result<SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>, CatalogError> {
    let path = fields.location(name);
    fields.array(name, notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                element(
                    fields.context(),
                    value,
                    fields.source().type_name(),
                    &format!("{path}[{ordinal}]"),
                    notes,
                )
            })
            .collect::<Result<Vec<_>, CatalogError>>()
    })
}

fn element(
    context: &CatalogRecordContext<'_>,
    value: &Value<'_>,
    owner: &str,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
    let reference = match value {
        Value::Reference(target) if target.location().is_none() => {
            let mut reference = CatalogRecordReference::resolved(
                context,
                path,
                target.record_index(),
                CatalogGuid(target.guid()),
                target.record_path(),
                target.type_name(),
            )?;
            if target.type_name() != "Tag" {
                reference.status = DefinitionStatus::UnexpectedTargetType;
                notes.push(FieldError::Malformed {
                    source_type: owner.to_owned(),
                    field: path.to_owned(),
                    expected: "a Tag Record Reference",
                });
            }
            Some(reference)
        }
        Value::Error(ValueError::DanglingReference { guid }) => {
            Some(CatalogRecordReference::unresolved(
                context,
                path,
                CatalogGuid(*guid),
                DefinitionStatus::Dangling,
            )?)
        }
        Value::Error(ValueError::UnexpectedReferenceType { guid, .. }) => {
            Some(CatalogRecordReference::unresolved(
                context,
                path,
                CatalogGuid(*guid),
                DefinitionStatus::UnexpectedTargetType,
            )?)
        }
        _ => None,
    };
    reference.map_or_else(
        || {
            context.read(
                Some(value),
                owner,
                path,
                "a nullable Tag Record Reference",
                notes,
                |_| None,
            )
        },
        |reference| context.value(path, SourceValue::Present(reference)),
    )
}
