// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, SourceUse, SourceUseRole, SourceValue, SourceValueState, SourcedValue, Value,
    ValueError,
};

use super::{Fact, Observed};
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::{OriginalDatum, OriginalValuePart};
use crate::catalog_v3::authored_fields::AuthoredRestriction;
use crate::resource_number::ResourceNumber;
use crate::source_fields::{FieldError, FieldViolation};
use crate::source_value_catalog::{self, SourceArenaOutput};
use crate::{CatalogError, ExportAccountingError};

#[path = "authored_enum_fields.rs"]
mod enumeration;
#[path = "authored_owned_family.rs"]
mod family;

#[derive(Clone, Copy)]
enum ClassShape {
    Scalar,
    Array,
}

impl ClassShape {
    const fn description(self) -> &'static str {
        match self {
            Self::Scalar => "a nullable scalar Class",
            Self::Array => "a nullable Class array",
        }
    }
}

impl<'reader, 'scope, 'read, 'source> Observed<'reader, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn declared_class<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.read_class(
            name,
            ClassShape::Scalar,
            None,
            notes,
            |fact, expected, notes| self.owned_state(name, fact, expected, notes, decode),
        )
    }

    /// Reads an inline class bound to the expected original declaration.
    pub(in crate::catalog_v3) fn declared_class_to<T>(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.read_class(
            name,
            ClassShape::Scalar,
            Some(target),
            notes,
            |fact, expected, notes| self.owned_state(name, fact, expected, notes, decode),
        )
    }

    pub(in crate::catalog_v3) fn declared_class_array<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.read_class(
            name,
            ClassShape::Array,
            None,
            notes,
            |fact, expected, notes| {
                let state = self.array_state(name, fact, expected, notes, decode)?;
                Ok((OriginalValuePart::Value, state))
            },
        )
    }

    /// Reads original owned slots stored inline or as pointers to their declared family.
    pub(in crate::catalog_v3) fn declared_owned_array<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        if storage.is_array() && storage.data_type() == DataType::StrongPointer {
            return self.declared_pointer_array(name, fact, notes, decode);
        }
        self.declared_class_array(name, notes, decode)
    }

    fn read_class<T>(
        &self,
        name: &str,
        shape: ClassShape,
        target: Option<&str>,
        notes: &mut Vec<FieldError>,
        read: impl FnOnce(
            &'reader Fact<'scope, 'read, 'source>,
            &[&str],
            &mut Vec<FieldError>,
        ) -> Result<(OriginalValuePart, SourceValue<T>), CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        if storage.data_type() != DataType::Class
            || storage.is_array() != matches!(shape, ClassShape::Array)
        {
            let state = self.failure(name, fact.value(), shape.description(), notes)?;
            return self.bind_storage(fact, state);
        }
        let declared = fact.observe_declared_target(self.input)?;
        let target = target.or_else(|| declared.0.as_ref().ok().copied());
        self.read_declared_value(fact, target.as_slice(), declared, notes, |notes| {
            let (part, state) = read(fact, target.as_slice(), notes)?;
            fact.bind(part, state, SourceUseRole::Selector, true)
        })
    }
    /// Retains the actual contents of a scalar `StrongPointer` to the declared base.
    pub(in crate::catalog_v3) fn raw_strong_pointer_to(
        &self,
        name: &str,
        declared_base: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<SourceArenaOutput<'source>>, CatalogError> {
        self.strong_pointer_family_to(name, declared_base, notes, |source, _| {
            let owner = source
                .owner
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            source_value_catalog::read_subtree(source.originals, owner, |original| {
                source.retain_raw_number(original)
            })
        })
    }

    pub(in crate::catalog_v3) fn authored_members(
        &self,
    ) -> Result<SourceArenaOutput<'source>, CatalogError> {
        self.authored_members_except([])
    }

    /// Retains original members outside the first occurrences represented by typed cells.
    pub(in crate::catalog_v3) fn authored_members_except(
        &self,
        projected: impl IntoIterator<Item = (&'static str, SourceValueState)>,
    ) -> Result<SourceArenaOutput<'source>, CatalogError> {
        let projected = projected.into_iter();
        let mut consumed = Vec::with_capacity(projected.size_hint().0);
        for (name, state) in projected {
            if matches!(state, SourceValueState::Present | SourceValueState::Null)
                && let Some((_, member)) = self.originals.field(self.owner, name)?
            {
                consumed.push(member.ordinal());
            }
        }
        source_value_catalog::read_members(self.originals, self.owner, &consumed, |original| {
            self.retain_raw_number(original)
        })
    }

    /// Retains one exact authored member without narrowing its storage or payload type.
    pub(in crate::catalog_v3) fn authored_member(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<SourceArenaOutput<'source>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let state = match fact.value() {
            Value::Null => SourceValue::Null,
            value @ Value::Error(_) => {
                self.failure(name, value, "a readable authored member", notes)?
            }
            _ => SourceValue::Present(source_value_catalog::read_subtree(
                self.originals,
                fact,
                |child| self.retain_raw_number(child),
            )?),
        };
        self.bind_field(
            fact,
            OriginalValuePart::Value,
            state,
            SourceUseRole::Selector,
        )
    }

    pub(in crate::catalog_v3) fn restriction_label(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<AuthoredRestriction<'source, &'source str>>, CatalogError> {
        self.restriction(name, notes, |value| match value {
            Value::String(label) | Value::Enum(label) => Some(*label),
            _ => None,
        })
    }

    pub(in crate::catalog_v3) fn restriction_number(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<AuthoredRestriction<'source, ResourceNumber>>, CatalogError> {
        self.restriction(name, notes, ResourceNumber::read)
    }

    fn restriction<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(&Value<'source>) -> Option<T>,
    ) -> Result<SourcedValue<AuthoredRestriction<'source, T>>, CatalogError> {
        self.scalar(name, "a readable original restriction", notes, |fact| {
            let value = fact.value();
            if matches!(value, Value::Null | Value::Error(_)) {
                return Ok(None);
            }
            let retained = if let Some(typed) = decode(value) {
                self.retain_raw_number(fact)?;
                AuthoredRestriction::Typed(typed)
            } else {
                AuthoredRestriction::Original(source_value_catalog::read_subtree(
                    self.originals,
                    fact,
                    |original| self.retain_raw_number(original),
                )?)
            };
            Ok(Some(retained))
        })
    }

    fn retain_raw_number(
        &self,
        original: &Fact<'scope, 'read, 'source>,
    ) -> Result<(), CatalogError> {
        if let Some(number) =
            CapturedNumber::authored_raw(self.originals, original, self.number_owner)?
        {
            self.numbers.borrow_mut().push(number);
        }
        Ok(())
    }

    pub(super) fn read_declared_owned<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        expected: &[&str],
        notes: &mut Vec<FieldError>,
        read: impl FnOnce(
            &mut Vec<FieldError>,
        ) -> Result<(OriginalValuePart, SourceValue<T>), CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let declared = fact.observe_declared_target(self.input)?;
        self.read_declared_value(fact, expected, declared, notes, |notes| {
            let (part, state) = read(notes)?;
            fact.bind(part, state, SourceUseRole::Selector, true)
        })
    }

    fn read_declared_value<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        expected: &[&str],
        (declared, [field, data_type, is_array, target, declaration]): (
            Result<&str, ValueError>,
            [SourceUse; 5],
        ),
        notes: &mut Vec<FieldError>,
        read: impl FnOnce(&mut Vec<FieldError>) -> Result<OriginalDatum<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let selectors = [
            self.kind()?,
            field,
            data_type,
            is_array,
            target,
            declaration,
        ];
        let sources = self.originals.sources();
        match declared {
            Ok(name) if expected.contains(&name) => {
                let mut value = read(notes)?;
                for selector in selectors {
                    value.contribute(selector);
                }
                Ok(value.into_cell(sources))
            }
            other => {
                let error = other.err();
                self.rejected_declaration(fact, error, notes)?;
                let state = if error.is_none() {
                    SourceValue::Unsupported
                } else {
                    SourceValue::Unreadable
                };
                Ok(sources.value(state, &selectors, error.is_none()))
            }
        }
    }

    fn rejected_declaration(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        error: Option<ValueError>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        let path = self.owner.map_or_else(
            || Ok(String::new()),
            |owner| self.originals.logical_path(owner),
        )?;
        let violation =
            FieldViolation::rejected_declaration(self.structure()?, &path, fact.field(), error);
        let body = self.capture.field_failure(&violation)?;
        notes.push(violation.into_error().with_body(body));
        Ok(())
    }
}
