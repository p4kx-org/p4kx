// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::FieldError;

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
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        let state = if storage.data_type() == DataType::Class && storage.is_array() {
            self.array_state(name, fact, types, notes, decode)?
        } else {
            self.failure(name, fact.value(), "a nullable Class array", notes)?
        };
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
        Ok(match fact.value() {
            Value::Null => SourceValue::Null,
            Value::Array(values) => {
                let mut rows = Vec::with_capacity(values.len());
                for (index, original) in values.iter().enumerate() {
                    let element = self.originals.fact(original)?;
                    let path = format!("{name}[{index}]");
                    let (part, state) =
                        self.owned_state(&path, element, types, notes, &mut decode)?;
                    let mut cell = element.bind(part, state, SourceUseRole::Selector, true)?;
                    cell.contribute(self.kind()?);
                    rows.push(cell.into_cell(self.originals.sources()));
                }
                SourceValue::Present(rows)
            }
            value => self.failure(name, value, "a nullable Array", notes)?,
        })
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
