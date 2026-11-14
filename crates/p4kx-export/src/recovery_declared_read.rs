// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, FieldStorage, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};

use super::RecoveryReader;
use crate::CatalogError;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::{FieldError, FieldViolation};

impl<'read, 'source> RecoveryReader<'read, 'source, '_> {
    pub(super) fn owned_structure<'fields>(
        &'fields self,
        parent: &CatalogFields<'fields, 'source, '_>,
        name: &str,
        expected: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'fields StructValue<'source>>, CatalogError> {
        if let Some(value) =
            self.read_declared(parent, name, expected, notes, |storage, value| {
                if storage.data_type() != DataType::StrongPointer || storage.is_array() {
                    return None;
                }
                match value {
                    Value::Struct(source) if source.type_name() == expected => {
                        Some(SourceValue::Present(source))
                    }
                    Value::Null => Some(SourceValue::Null),
                    _ => None,
                }
            })?
        {
            return Ok(value);
        }
        parent.structure(name, notes)
    }

    pub(in super::super) fn empty_class_target<'fields>(
        &'fields self,
        parent: &CatalogFields<'fields, 'source, '_>,
        name: &str,
        expected: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'fields [Value<'source>]>, CatalogError> {
        if let Some(value) = self.read_declared(
            parent,
            name,
            expected,
            notes,
            |storage, value| match value {
                Value::Array(elements)
                    if storage.data_type() == DataType::Class
                        && storage.is_array()
                        && elements.is_empty() =>
                {
                    Some(SourceValue::Present(elements.as_ref()))
                }
                _ => None,
            },
        )? {
            return Ok(value);
        }
        parent.array(name, notes)
    }

    fn read_declared<T>(
        &self,
        parent: &CatalogFields<'_, 'source, '_>,
        name: &str,
        expected: &str,
        notes: &mut Vec<FieldError>,
        selected: impl FnOnce(FieldStorage, &'read Value<'source>) -> Option<SourceValue<T>>,
    ) -> Result<Option<SourcedValue<T>>, CatalogError> {
        let owner = self.original.owner_of(parent.source())?;
        let Some((_, fact)) = self.original.field(owner, name)? else {
            return Ok(None);
        };
        let Some(value) = selected(fact.field().storage(), fact.value()) else {
            return Ok(None);
        };
        let (target, selectors) = fact.observe_declared_target(self.input)?;
        if target == Ok(expected) {
            let mut bound = fact.bind(
                OriginalValuePart::Value,
                value,
                SourceUseRole::Payload,
                true,
            )?;
            for selector in selectors {
                bound.contribute(selector);
            }
            return Ok(Some(bound.into_cell(self.original.sources())));
        }
        let path = owner
            .map(|owner| self.original.logical_path(owner))
            .transpose()?
            .unwrap_or_default();
        let error = target.err();
        let violation =
            FieldViolation::rejected_declaration(parent.source(), &path, fact.field(), error);
        let body = self.capture.field_failure(&violation)?;
        notes.push(violation.into_error().with_body(body));
        let state = if error.is_none() {
            SourceValue::Unsupported
        } else {
            SourceValue::Unreadable
        };
        Ok(Some(self.original.sources().value(
            state,
            &selectors,
            error.is_none(),
        )))
    }
}
