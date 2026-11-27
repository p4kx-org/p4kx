// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::FieldError;

#[path = "authored_owned_declaration.rs"]
mod declaration;

enum DeclarationCheck<'types> {
    EmptyOnly,
    Always(&'types [&'types str]),
}

impl<'types> DeclarationCheck<'types> {
    fn targets(
        self,
        value: &Value<'_>,
        concrete: &'types [&'types str],
    ) -> Option<&'types [&'types str]> {
        match self {
            Self::EmptyOnly if matches!(value, Value::Array(values) if values.is_empty()) => {
                Some(concrete)
            }
            Self::EmptyOnly => None,
            Self::Always(declared) => Some(declared),
        }
    }
}

#[derive(Clone, Copy)]
enum OwnedStorage {
    Class,
    StrongPointer,
}

impl OwnedStorage {
    const fn data_type(self) -> DataType {
        match self {
            Self::Class => DataType::Class,
            Self::StrongPointer => DataType::StrongPointer,
        }
    }

    const fn expectation(self) -> &'static str {
        match self {
            Self::Class => "a nullable Class",
            Self::StrongPointer => "a nullable StrongPointer",
        }
    }
}

impl<'reader, 'scope, 'read, 'source> Observed<'reader, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn owned<T>(
        &self,
        name: &str,
        expected: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let (part, state) = self.owned_state(name, fact, expected, notes, decode)?;
        self.bind_field(fact, part, state, SourceUseRole::Selector)
    }

    pub(in crate::catalog_v3) fn inline_class<T>(
        &self,
        name: &str,
        expected: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.typed_owned(name, expected, OwnedStorage::Class, notes, decode)
    }

    pub(in crate::catalog_v3) fn strong_pointer<T>(
        &self,
        name: &str,
        expected: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.typed_owned(name, expected, OwnedStorage::StrongPointer, notes, decode)
    }

    pub(in crate::catalog_v3) fn strong_pointer_to<T>(
        &self,
        name: &str,
        target: &str,
        concrete: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        if storage.data_type() != DataType::StrongPointer || storage.is_array() {
            let state =
                self.failure(name, fact.value(), "a nullable scalar StrongPointer", notes)?;
            return self.bind_storage(fact, state);
        }
        self.read_declared_owned(fact, &[target], notes, |notes| {
            self.owned_state(name, fact, concrete, notes, decode)
        })
    }

    fn typed_owned<T>(
        &self,
        name: &str,
        expected: &[&str],
        storage: OwnedStorage,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let (part, state) = if fact.field().storage().data_type() == storage.data_type() {
            self.owned_state(name, fact, expected, notes, decode)?
        } else {
            (
                OriginalValuePart::Value,
                self.failure(name, fact.value(), storage.expectation(), notes)?,
            )
        };
        let mut cell = self.field_datum(fact, part, state, SourceUseRole::Selector)?;
        cell.contribute(fact.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?);
        Ok(cell.into_cell(self.originals.sources()))
    }

    pub(in crate::catalog_v3) fn array<T>(
        &self,
        name: &str,
        types: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let state = self.array_state(name, fact, types, notes, decode)?;
        self.bind_field(
            fact,
            OriginalValuePart::Value,
            state,
            SourceUseRole::Selector,
        )
    }

    pub(in crate::catalog_v3) fn class_array<T>(
        &self,
        name: &str,
        types: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.read_owned_array(
            name,
            types,
            OwnedStorage::Class,
            DeclarationCheck::EmptyOnly,
            notes,
            decode,
        )
    }

    pub(in crate::catalog_v3) fn class_array_to<T>(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.read_owned_array(
            name,
            &[target],
            OwnedStorage::Class,
            DeclarationCheck::Always(&[target]),
            notes,
            decode,
        )
    }

    pub(in crate::catalog_v3) fn strong_pointer_array_to<T>(
        &self,
        name: &str,
        target: &str,
        concrete: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.read_owned_array(
            name,
            concrete,
            OwnedStorage::StrongPointer,
            DeclarationCheck::Always(&[target]),
            notes,
            decode,
        )
    }

    fn read_owned_array<T>(
        &self,
        name: &str,
        types: &[&str],
        storage: OwnedStorage,
        check: DeclarationCheck<'_>,
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let declaration = fact.field().storage();
        if declaration.data_type() != storage.data_type() || !declaration.is_array() {
            let expected = match storage {
                OwnedStorage::Class => "a nullable Class array",
                OwnedStorage::StrongPointer => "a nullable StrongPointer array",
            };
            let state = self.failure(name, fact.value(), expected, notes)?;
            return self.bind_storage(fact, state);
        }
        if let Some(declared_types) = check.targets(fact.value(), types) {
            return self.read_declared_owned(fact, declared_types, notes, |notes| {
                let state = self.array_state(name, fact, types, notes, decode)?;
                Ok((OriginalValuePart::Value, state))
            });
        }
        let state = self.array_state(name, fact, types, notes, decode)?;
        self.bind_storage(fact, state)
    }

    pub(super) fn bind_storage<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        state: SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut cell = self.field_datum(
            fact,
            OriginalValuePart::Value,
            state,
            SourceUseRole::Selector,
        )?;
        for part in [OriginalValuePart::DataType, OriginalValuePart::IsArray] {
            cell.contribute(fact.observe(part, SourceUseRole::Selector)?);
        }
        Ok(cell.into_cell(self.originals.sources()))
    }

    fn array_state<T>(
        &self,
        name: &str,
        fact: &'reader Fact<'scope, 'read, 'source>,
        types: &[&str],
        notes: &mut Vec<FieldError>,
        mut decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourceValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.array_elements(
            fact,
            name,
            "a nullable Array",
            notes,
            |index, element, notes| {
                let path = format!("{name}[{index}]");
                let (part, state) = self.owned_state(&path, element, types, notes, &mut decode)?;
                let mut cell = element.bind(part, state, SourceUseRole::Selector, true)?;
                cell.contribute(self.kind()?);
                Ok(cell.into_cell(self.originals.sources()))
            },
        )
    }

    fn owned_state<T>(
        &self,
        name: &str,
        fact: &'reader Fact<'scope, 'read, 'source>,
        types: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<(OriginalValuePart, SourceValue<T>), CatalogError> {
        Ok(match fact.value() {
            Value::Null => (OriginalValuePart::Value, SourceValue::Null),
            Value::Struct(source) if types.contains(&source.type_name()) => (
                OriginalValuePart::ConcreteType,
                SourceValue::Present(decode(
                    Self {
                        owner: Some(fact),
                        ..*self
                    },
                    notes,
                )?),
            ),
            Value::Struct(source) => (
                OriginalValuePart::ConcreteType,
                self.unsupported(fact, source, notes)?,
            ),
            other => (
                OriginalValuePart::Value,
                self.failure(name, other, "a nullable Struct", notes)?,
            ),
        })
    }
}
