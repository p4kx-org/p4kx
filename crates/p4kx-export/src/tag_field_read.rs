// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceValue, SourcedValue, Value, ValueError};

use super::path_capture::NameEvidence;
use crate::CatalogError;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::export_ledger::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

pub(super) fn name<'source>(
    fields: &CatalogFields<'_, 'source, '_>,
    original: &OriginalFlowFields<'_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<(SourcedValue<&'source str>, NameEvidence), CatalogError> {
    if fields.source().field("tagName").is_some() {
        let value = fields.text("tagName", notes)?;
        let state = value.state();
        return Ok((value, NameEvidence::Declared(state)));
    }
    if original.owner_of(fields.source())?.is_some() {
        return Err(ExportAccountingError::ConflictingTagTrace.into());
    }
    let survey = original.survey(None)?;
    let value = original
        .sources()
        .value(SourceValue::Absent, std::slice::from_ref(&survey), true);
    Ok((value, NameEvidence::Absent(survey)))
}

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
    fields.array(name, notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                element(fields, value, &format!("{name}[{ordinal}]"), notes)
            })
            .collect::<Result<Vec<_>, CatalogError>>()
    })
}

fn element<'s, 'f>(
    fields: &CatalogFields<'s, 'f, '_>,
    value: &'s Value<'f>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
    let reference = match value {
        Value::Reference(target) if target.location().is_none() => {
            let mut reference = CatalogRecordReference::resolved(
                fields.context(),
                &fields.location(path),
                target.record_index(),
                CatalogGuid(target.guid()),
                target.record_path(),
                target.type_name(),
            )?;
            if target.type_name() != "Tag" {
                reference.status = DefinitionStatus::UnexpectedTargetType;
                notes.push(fields.invalid(path, value, "a Tag Record Reference")?);
            }
            Some(reference)
        }
        Value::Error(ValueError::DanglingReference { guid }) => {
            Some(CatalogRecordReference::unresolved(
                fields.context(),
                &fields.location(path),
                CatalogGuid(*guid),
                DefinitionStatus::Dangling,
            )?)
        }
        Value::Error(ValueError::UnexpectedReferenceType { guid, .. }) => {
            Some(CatalogRecordReference::unresolved(
                fields.context(),
                &fields.location(path),
                CatalogGuid(*guid),
                DefinitionStatus::UnexpectedTargetType,
            )?)
        }
        _ => None,
    };
    reference.map_or_else(
        || {
            fields.read_value(
                Some(value),
                path,
                "a nullable Tag Record Reference",
                notes,
                |_| None,
            )
        },
        |reference| {
            fields
                .context()
                .value(&fields.location(path), SourceValue::Present(reference))
        },
    )
}
