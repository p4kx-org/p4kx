// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::FieldError;

#[derive(Clone, Copy)]
enum EnumShape {
    Scalar,
    Array,
}

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    /// Reads a declared Enum label or a directly stored String label.
    pub(in crate::catalog_v3) fn borrowed_enum_or_string(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.enum_field(name, target, true, notes)
    }

    /// Reads one scalar label with its exact original Enum declaration.
    pub(in crate::catalog_v3) fn borrowed_enum_to(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.enum_field(name, target, false, notes)
    }

    fn enum_field(
        &self,
        name: &str,
        target: &str,
        allow_string: bool,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        if allow_string && storage.data_type() == DataType::String && !storage.is_array() {
            return self.borrowed_string(name, notes);
        }
        self.declared_enum(name, target, EnumShape::Scalar, fact, notes, |notes| {
            self.enum_state(name, fact, notes)
        })
    }

    /// Reads ordered labels with an exact original Enum array declaration.
    pub(in crate::catalog_v3) fn borrowed_enum_array_to(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<&'source str>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        self.declared_enum(name, target, EnumShape::Array, fact, notes, |notes| {
            self.array_elements(
                fact,
                name,
                "a nullable Enum array",
                notes,
                |index, element, notes| {
                    let state = self.enum_state(&format!("{name}[{index}]"), element, notes)?;
                    let role = enum_role(&state);
                    self.bind_field(element, OriginalValuePart::Value, state, role)
                },
            )
        })
    }

    fn declared_enum<T>(
        &self,
        name: &str,
        target: &str,
        shape: EnumShape,
        fact: &Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        read: impl FnOnce(&mut Vec<FieldError>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let storage = fact.field().storage();
        if storage.data_type() != DataType::Enum
            || storage.is_array() != matches!(shape, EnumShape::Array)
        {
            let expected = match shape {
                EnumShape::Scalar => "a nullable scalar Enum",
                EnumShape::Array => "a nullable Enum array",
            };
            let state = self.failure(name, fact.value(), expected, notes)?;
            return self.bind_storage(fact, state);
        }
        fact.check_input(self.input)?;
        let (declared, declaration) = self.input.declared_enum_name(storage.target())?;
        self.read_declared_value(
            fact,
            &[target],
            (declared, fact.declaration_selectors(declaration)?),
            notes,
            |notes| {
                let state = read(notes)?;
                let role = match shape {
                    EnumShape::Scalar => enum_role(&state),
                    EnumShape::Array => SourceUseRole::Selector,
                };
                fact.bind(OriginalValuePart::Value, state, role, true)
            },
        )
    }

    fn enum_state(
        &self,
        name: &str,
        fact: &Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourceValue<&'source str>, CatalogError> {
        Ok(match fact.value() {
            Value::Enum(label) => SourceValue::Present(*label),
            Value::Null => SourceValue::Null,
            other => self.failure(name, other, "a nullable Enum", notes)?,
        })
    }
}

const fn enum_role<T>(state: &SourceValue<T>) -> SourceUseRole {
    if matches!(state, SourceValue::Present(_)) {
        SourceUseRole::Payload
    } else {
        SourceUseRole::Selector
    }
}
