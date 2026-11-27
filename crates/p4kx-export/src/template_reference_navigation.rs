// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use super::{TargetSelection, TargetSelectionKind, TemplateReferences};
use crate::catalog_read::original::{OriginalValueEdge, OriginalValueFact, OriginalValuePart};
use crate::{CatalogError, ExportAccountingError};

impl<'fields, 'scope, 'read, 'source> TemplateReferences<'fields, 'scope, 'read, 'source> {
    pub(crate) fn nested_field(
        &self,
        selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
        container: &str,
        name: &str,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        let mut result = self.select_field(selected, name)?;
        let TargetSelectionKind::Instance { owner, .. } = selected.kind else {
            return Ok(result);
        };
        let Some((_, declaration)) = self.fields.field(Some(owner), container)? else {
            result.kind = TargetSelectionKind::Instance { owner, field: None };
            return Ok(result);
        };
        result
            .selectors
            .push((declaration, OriginalValuePart::Value));
        result.retained_root = Some(declaration);
        let selected = match declaration.value() {
            Value::Struct(_) => Some(declaration),
            Value::Array(_) => {
                let mut first = None;
                for value in self.fields.elements(declaration)? {
                    result.selectors.push((value, OriginalValuePart::Value));
                    if matches!(value.value(), Value::Struct(_)) {
                        first = Some(value);
                        break;
                    }
                }
                first
            }
            _ => None,
        };
        result.kind = match selected {
            Some(owner) => TargetSelectionKind::Instance {
                owner,
                field: self
                    .fields
                    .field(Some(owner), name)?
                    .map(|(_, field)| field),
            },
            None => match declaration.value() {
                Value::Null => TargetSelectionKind::Null,
                Value::Error(_) => TargetSelectionKind::Unreadable,
                Value::Array(_) => TargetSelectionKind::Instance { owner, field: None },
                _ => TargetSelectionKind::WrongShape,
            },
        };
        Ok(result)
    }

    pub(crate) fn ancestor_field(
        &self,
        selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
        through: &str,
        name: &str,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        let mut result = self.select_field(selected, name)?;
        let TargetSelectionKind::Instance { owner, .. } = selected.kind else {
            return Ok(result);
        };
        let mut value = owner;
        while let Some(parent) = self.parent(value)? {
            if matches!(parent.edge(), OriginalValueEdge::Field { .. }) {
                result
                    .selectors
                    .push((parent, OriginalValuePart::FieldName));
                if parent.field().name() == through && matches!(parent.value(), Value::Array(_)) {
                    let Some(owner) = self.parent(parent)? else {
                        break;
                    };
                    if matches!(owner.value(), Value::Struct(_)) {
                        result.kind = TargetSelectionKind::Instance {
                            owner,
                            field: self
                                .fields
                                .field(Some(owner), name)?
                                .map(|(_, field)| field),
                        };
                        return Ok(result);
                    }
                }
            }
            value = parent;
        }
        result.kind = TargetSelectionKind::Unresolved;
        Ok(result)
    }

    fn parent(
        &self,
        value: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        self.fields.span(value)?;
        value
            .parent_ordinal()
            .map(|parent| {
                let index =
                    usize::try_from(parent).map_err(|_| ExportAccountingError::CounterOverflow)?;
                self.fields
                    .facts()
                    .get(index)
                    .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
            })
            .transpose()
    }
}
