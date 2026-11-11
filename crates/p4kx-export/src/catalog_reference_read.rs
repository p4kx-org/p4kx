// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue, Value, ValueError};

use super::CatalogRecordContext;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::source_fields::FieldError;

/// Shares target resolution while binding a failure to its actual field guard.
pub(crate) fn reference_with_failure<'source, 'field>(
    context: &CatalogRecordContext<'_>,
    value: Option<&'field Value<'source>>,
    path: &str,
    notes: &mut Vec<FieldError>,
    failure: impl FnOnce(&'field Value<'source>) -> Result<FieldError, CatalogError>,
) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
    let reference = match value {
        Some(Value::Reference(target)) if target.location().is_none() => {
            Some(CatalogRecordReference::resolved(
                context,
                path,
                target.record_index(),
                CatalogGuid(target.guid()),
                target.record_path(),
                target.type_name(),
            )?)
        }
        Some(Value::Error(ValueError::DanglingReference { guid })) => {
            Some(CatalogRecordReference::unresolved(
                context,
                path,
                CatalogGuid(*guid),
                DefinitionStatus::Dangling,
            )?)
        }
        Some(Value::Error(ValueError::UnexpectedReferenceType { guid, .. })) => {
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
        || context.read_with(value, path, notes, |_| None, failure),
        |reference| context.value(path, SourceValue::Present(reference)),
    )
}
