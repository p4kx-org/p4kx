// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::{
    OriginalDatum, OriginalFlowFields, OriginalValueFact, OriginalValuePart,
};
use crate::record_fields::number_value;
use crate::resource_float::ResourceFloat32;
use crate::source_fields::{FieldCapture, FieldError, FieldViolation};
use crate::{CatalogError, ExportAccountingError};

#[path = "authored_field_links.rs"]
mod links;

type Fact<'scope, 'read, 'source> = OriginalValueFact<'scope, 'read, 'source>;

pub(super) struct ReadHealth {
    pub(super) value: SourcedValue<ResourceFloat32>,
    pub(super) legacy_value: Option<f32>,
}

/// Reads typed members of one actual component or one of its owned structs.
#[derive(Clone, Copy)]
pub(super) struct Observed<'reader, 'scope, 'read, 'source> {
    originals: &'reader OriginalFlowFields<'scope, 'read, 'source>,
    owner: &'reader Fact<'scope, 'read, 'source>,
    capture: &'reader dyn FieldCapture<'source>,
    numbers: &'reader RefCell<Vec<CapturedNumber>>,
}

impl<'reader, 'scope, 'read, 'source> Observed<'reader, 'scope, 'read, 'source> {
    pub(super) const fn new(
        originals: &'reader OriginalFlowFields<'scope, 'read, 'source>,
        owner: &'reader Fact<'scope, 'read, 'source>,
        capture: &'reader dyn FieldCapture<'source>,
        numbers: &'reader RefCell<Vec<CapturedNumber>>,
    ) -> Self {
        Self {
            originals,
            owner,
            capture,
            numbers,
        }
    }

    pub(super) fn single(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
        self.single_observed(name, notes, |_| {})
    }

    pub(super) fn health(&self, notes: &mut Vec<FieldError>) -> Result<ReadHealth, CatalogError> {
        let mut legacy_value = None;
        let value = self.single_observed("Health", notes, |value| {
            legacy_value = number_value(value);
        })?;
        Ok(ReadHealth {
            value,
            legacy_value,
        })
    }

    fn single_observed(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        observed: impl FnOnce(&Value<'source>),
    ) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
        self.scalar(name, "a nullable Single", notes, |fact| {
            let value = fact.value();
            observed(value);
            let Value::Single(number) = value else {
                return Ok(None);
            };
            if let Some(number) = CapturedNumber::item_component(self.originals, fact)? {
                self.numbers.borrow_mut().push(number);
            }
            Ok(Some(ResourceFloat32::new(*number)))
        })
    }

    pub(super) fn boolean(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<bool>, CatalogError> {
        self.scalar(name, "a nullable Bool", notes, |fact| {
            Ok(match fact.value() {
                Value::Bool(value) => Some(*value),
                _ => None,
            })
        })
    }

    pub(super) fn int32(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<i32>, CatalogError> {
        self.scalar(name, "a nullable Int32", notes, |fact| {
            Ok(match fact.value() {
                Value::Int32(value) => Some(*value),
                _ => None,
            })
        })
    }

    pub(super) fn string(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<String>, CatalogError> {
        Ok(self.borrowed_string(name, notes)?.map(str::to_owned))
    }

    pub(super) fn borrowed_string(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.scalar(name, "a nullable String", notes, |fact| {
            Ok(match fact.value() {
                Value::String(text) => Some(*text),
                _ => None,
            })
        })
    }

    pub(super) fn enumeration(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<String>, CatalogError> {
        self.scalar(name, "a nullable Enum", notes, |fact| {
            Ok(match fact.value() {
                Value::Enum(text) => Some((*text).to_owned()),
                _ => None,
            })
        })
    }

    pub(super) fn source_type(&self) -> Result<&'source str, CatalogError> {
        self.structure().map(StructValue::type_name)
    }

    fn scalar<T>(
        &self,
        name: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<Option<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(Some(self.owner), name)? else {
            return self.absent();
        };
        let state = match fact.value() {
            Value::Null => SourceValue::Null,
            value => match decode(fact)? {
                Some(value) => SourceValue::Present(value),
                None => self.failure(name, value, expected, notes)?,
            },
        };
        let role = if matches!(state, SourceValue::Present(_)) {
            SourceUseRole::Payload
        } else {
            SourceUseRole::Selector
        };
        self.bind_field(fact, OriginalValuePart::Value, state, role)
    }

    pub(super) fn owned<T>(
        &self,
        name: &str,
        expected: &[&str],
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(Some(self.owner), name)? else {
            return self.absent();
        };
        let (part, state) = self.owned_state(name, fact, expected, notes, decode)?;
        self.bind_field(fact, part, state, SourceUseRole::Selector)
    }

    pub(super) fn array<T>(
        &self,
        name: &str,
        types: &[&str],
        notes: &mut Vec<FieldError>,
        mut decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(Some(self.owner), name)? else {
            return self.absent();
        };
        let state = match fact.value() {
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
        };
        self.bind_field(
            fact,
            OriginalValuePart::Value,
            state,
            SourceUseRole::Selector,
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
                    Self::new(self.originals, fact, self.capture, self.numbers),
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

    fn unsupported<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        source: &StructValue<'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourceValue<T>, CatalogError> {
        let path = self.originals.logical_path(fact)?;
        let body = self.capture.unsupported_type(source, &path)?;
        notes.push(
            FieldError::Unsupported {
                source_type: source.type_name().to_owned(),
                field: path,
            }
            .with_body(body),
        );
        Ok(SourceValue::Unsupported)
    }

    fn structure(&self) -> Result<&'read StructValue<'source>, CatalogError> {
        let Value::Struct(source) = self.owner.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        Ok(source)
    }

    fn failure<T>(
        &self,
        name: &str,
        value: &Value<'source>,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourceValue<T>, CatalogError> {
        let path = self.originals.logical_path(self.owner)?;
        let violation = FieldViolation::invalid(self.structure()?, &path, name, value, expected);
        let body = self.capture.field_failure(&violation)?;
        notes.push(violation.into_error().with_body(body));
        Ok(if matches!(value, Value::Error(_)) {
            SourceValue::Unreadable
        } else {
            SourceValue::Invalid
        })
    }

    fn absent<T>(&self) -> Result<SourcedValue<T>, CatalogError> {
        Ok(self.originals.sources().value(
            SourceValue::Absent,
            &[self.kind()?, self.originals.survey(Some(self.owner))?],
            true,
        ))
    }

    fn bind_field<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        part: OriginalValuePart,
        state: SourceValue<T>,
        role: SourceUseRole,
    ) -> Result<SourcedValue<T>, CatalogError> {
        Ok(self
            .field_datum(fact, part, state, role)?
            .into_cell(self.originals.sources()))
    }

    fn field_datum<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        part: OriginalValuePart,
        state: SourceValue<T>,
        role: SourceUseRole,
    ) -> Result<OriginalDatum<T>, CatalogError> {
        let mut cell = fact.bind(part, state, role, true)?;
        cell.contribute(self.kind()?);
        cell.contribute(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        Ok(cell)
    }

    fn kind(&self) -> Result<SourceUse, CatalogError> {
        self.owner
            .observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)
    }
}
