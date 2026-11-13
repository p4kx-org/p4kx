// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    DecimalU64, SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::{AuthoredNumberOwner, CapturedNumber};
use crate::catalog_read::original::{
    OriginalDatum, OriginalFlowFields, OriginalValueFact, OriginalValuePart,
};
use crate::record_fields::number_value;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::source_fields::{FieldCapture, FieldError, FieldViolation};
use crate::{CatalogError, ExportAccountingError};

#[path = "authored_alignment.rs"]
mod alignment;
#[path = "authored_array.rs"]
mod array;
#[path = "authored_float_fields.rs"]
mod float_fields;
#[path = "authored_field_links.rs"]
mod links;
#[path = "authored_owned_fields.rs"]
mod owned_fields;
#[path = "authored_room_offset.rs"]
mod room_offset;
pub(super) use room_offset::{RoomOffset, RoomTransformUnavailable};
#[path = "authored_text_fields.rs"]
mod text_fields;
#[path = "authored_unsigned_fields.rs"]
mod unsigned;

type Fact<'scope, 'read, 'source> = OriginalValueFact<'scope, 'read, 'source>;

/// A stored restriction with its typed value or exact original storage shape.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "AuthoredRestrictionOf{T}")]
pub(super) enum AuthoredRestriction<'source, T> {
    Typed(T),
    Original(crate::source_value_catalog::SourceArenaOutput<'source>),
}

pub(super) struct ReadHealth {
    pub(super) value: SourcedValue<ResourceFloat32>,
    pub(super) legacy_value: Option<f32>,
}

/// Reads typed members in one admitted Record or owned struct.
#[derive(Clone, Copy)]
pub(super) struct Observed<'reader, 'scope, 'read, 'source> {
    originals: &'reader OriginalFlowFields<'scope, 'read, 'source>,
    input: &'reader OriginalCatalogInput<'reader, 'source>,
    owner: Option<&'reader Fact<'scope, 'read, 'source>>,
    capture: &'reader dyn FieldCapture<'source>,
    numbers: &'reader RefCell<Vec<CapturedNumber>>,
    number_owner: AuthoredNumberOwner,
}

impl<'reader, 'scope, 'read, 'source> Observed<'reader, 'scope, 'read, 'source> {
    pub(super) const fn new(
        originals: &'reader OriginalFlowFields<'scope, 'read, 'source>,
        input: &'reader OriginalCatalogInput<'reader, 'source>,
        owner: &'reader Fact<'scope, 'read, 'source>,
        capture: &'reader dyn FieldCapture<'source>,
        numbers: &'reader RefCell<Vec<CapturedNumber>>,
    ) -> Self {
        Self {
            originals,
            input,
            owner: Some(owner),
            capture,
            numbers,
            number_owner: AuthoredNumberOwner::ItemComponent,
        }
    }

    pub(super) const fn definition(
        originals: &'reader OriginalFlowFields<'scope, 'read, 'source>,
        input: &'reader OriginalCatalogInput<'reader, 'source>,
        capture: &'reader dyn FieldCapture<'source>,
        numbers: &'reader RefCell<Vec<CapturedNumber>>,
    ) -> Self {
        Self {
            originals,
            input,
            owner: None,
            capture,
            numbers,
            number_owner: AuthoredNumberOwner::Definition,
        }
    }

    pub(super) const fn at_owner(self, owner: &'reader Fact<'scope, 'read, 'source>) -> Self {
        Self {
            owner: Some(owner),
            ..self
        }
    }

    pub(super) const fn configuration(
        originals: &'reader OriginalFlowFields<'scope, 'read, 'source>,
        input: &'reader OriginalCatalogInput<'reader, 'source>,
        capture: &'reader dyn FieldCapture<'source>,
        numbers: &'reader RefCell<Vec<CapturedNumber>>,
    ) -> Self {
        Self {
            number_owner: AuthoredNumberOwner::Configuration,
            ..Self::definition(originals, input, capture, numbers)
        }
    }

    pub(super) fn authored_number(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        self.scalar(name, "a nullable number", notes, |fact| {
            let value = ResourceNumber::read(fact.value());
            if let Some(number) =
                CapturedNumber::authored_raw(self.originals, fact, self.number_owner)?
            {
                self.numbers.borrow_mut().push(number);
            }
            Ok(value)
        })
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

    pub(super) fn int64(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<i64>, CatalogError> {
        self.scalar(name, "a nullable Int64", notes, |fact| {
            Ok(match fact.value() {
                Value::Int64(value) => Some(*value),
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

    pub(super) fn has_member(&self, name: &str) -> Result<bool, CatalogError> {
        Ok(self.originals.field(self.owner, name)?.is_some())
    }

    pub(super) fn check_cancel(&self) -> Result<(), CatalogError> {
        self.originals.check_cancel()
    }

    pub(super) fn source_type(&self) -> Result<&'source str, CatalogError> {
        self.structure().map(StructValue::type_name)
    }

    pub(super) fn owner_ordinal(&self) -> Result<SourcedValue<DecimalU64>, CatalogError> {
        let owner = self
            .owner
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.bind_field(
            owner,
            OriginalValuePart::Value,
            SourceValue::Present(DecimalU64::new(owner.ordinal())),
            SourceUseRole::Conversion,
        )
    }

    pub(super) fn array_ordinal(&self) -> Result<SourcedValue<u32>, CatalogError> {
        let owner = self
            .owner
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        super::item_component_owner::ComponentOwner::ordinal(self.originals, owner)
    }

    pub(super) fn source_type_value(&self) -> Result<SourcedValue<&'source str>, CatalogError> {
        Ok(self.originals.sources().value(
            SourceValue::Present(self.source_type()?),
            &[self.kind()?],
            true,
        ))
    }

    fn scalar<T>(
        &self,
        name: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<Option<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
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

    fn unsupported<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        source: &StructValue<'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourceValue<T>, CatalogError> {
        notes.push(self.variant_note(fact, source, |source_type, field| {
            FieldError::Unsupported { source_type, field }
        })?);
        Ok(SourceValue::Unsupported)
    }

    pub(super) fn note_retained_variant(
        &self,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        let owner = self
            .owner
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        notes.push(
            self.variant_note(owner, self.structure()?, |source_type, field| {
                FieldError::RetainedVariant { source_type, field }
            })?,
        );
        Ok(())
    }

    fn variant_note(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        source: &StructValue<'source>,
        note: impl FnOnce(String, String) -> FieldError,
    ) -> Result<FieldError, CatalogError> {
        let path = self.originals.logical_path(fact)?;
        let body = self.capture.unsupported_type(source, &path)?;
        Ok(note(source.type_name().to_owned(), path).with_body(body))
    }

    fn structure(&self) -> Result<&'read StructValue<'source>, CatalogError> {
        let Some(owner) = self.owner else {
            return Ok(self.originals.root());
        };
        let Value::Struct(source) = owner.value() else {
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
        let path = self.owner.map_or_else(
            || Ok(String::new()),
            |owner| self.originals.logical_path(owner),
        )?;
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
            &[self.kind()?, self.originals.survey(self.owner)?],
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
        self.owner.map_or_else(
            || self.originals.observe_type(self.originals.root()),
            |owner| owner.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector),
        )
    }
}
