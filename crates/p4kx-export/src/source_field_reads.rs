// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use super::reference_shape::{self, ReferenceShape, ReferenceSlot};
use super::violation::FieldViolation;

/// The single borrowed source guard implementation used by plain and observed callers.
#[derive(Clone, Copy)]
pub(super) struct FieldReads<'scope, 'source, 'path> {
    source: &'scope StructValue<'source>,
    path: &'path str,
}

impl<'scope, 'source, 'path> FieldReads<'scope, 'source, 'path> {
    pub(super) const fn new(source: &'scope StructValue<'source>, path: &'path str) -> Self {
        Self { source, path }
    }

    pub(super) fn text<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<&'source str>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable String", |value| match value {
            Value::String(value) => Some(*value),
            _ => None,
        })
    }

    pub(super) fn locale<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<&'source str>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable Locale", |value| match value {
            Value::Locale(value) => Some(*value),
            _ => None,
        })
    }

    pub(super) fn boolean<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<bool>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable Bool", |value| match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        })
    }

    pub(super) fn int32<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<i32>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable Int32", |value| match value {
            Value::Int32(value) => Some(*value),
            _ => None,
        })
    }

    pub(super) fn single<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<f32>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable finite Single", |value| match value {
            Value::Single(value) if value.is_finite() => Some(*value),
            _ => None,
        })
    }

    pub(super) fn number<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<f32>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(
            name,
            "a finite number representable as a 32-bit float",
            |_| crate::record_fields::number(Some(self.source), name),
        )
    }

    pub(super) fn structure<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<&'scope StructValue<'source>>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable owned struct", |value| match value {
            Value::Struct(value) => Some(value),
            _ => None,
        })
    }

    pub(super) fn array<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<&'scope [Value<'source>]>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.optional(name, "a nullable array", |value| match value {
            Value::Array(value) => Some(value.as_ref()),
            _ => None,
        })
    }

    pub(super) fn reference<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<ReferenceShape<'scope, 'source>>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        self.reference_slot(name)
            .map(|slot| slot.map(|slot| slot.reference))
    }

    pub(super) fn reference_slot<'error>(
        self,
        name: &'error str,
    ) -> Result<Option<ReferenceSlot<'scope, 'source>>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        let Some(value) = self.source.field(name) else {
            return Ok(None);
        };
        let reference = reference_shape::read(value).map_err(|expected| {
            FieldViolation::invalid(self.source, self.path, name, value, expected)
        })?;
        Ok(reference.map(|reference| ReferenceSlot { value, reference }))
    }

    fn optional<'error, T>(
        self,
        name: &'error str,
        expected: &'static str,
        read: impl FnOnce(&'scope Value<'source>) -> Option<T>,
    ) -> Result<Option<T>, FieldViolation<'error, 'source>>
    where
        'scope: 'error,
        'path: 'error,
    {
        match self.source.field(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => read(value).map(Some).ok_or_else(|| {
                FieldViolation::invalid(self.source, self.path, name, value, expected)
            }),
        }
    }
}
