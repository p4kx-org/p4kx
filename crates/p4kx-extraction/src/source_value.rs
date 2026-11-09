// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::field_origin::FieldOrigin;
use crate::source_value_schema;

/// A source value before its physical contributions are bound.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SourceValue<T> {
    /// A present value, including zero, false, empty collections and nonfinite IEEE values.
    Present(T),
    /// The source did not declare this field.
    Absent,
    /// The source explicitly declared a null value.
    Null,
    /// The declared value could not be read.
    Unreadable,
    /// The source shape is outside the supported typed payload.
    Unsupported,
    /// The source value is incompatible with the required typed field.
    Invalid,
}

/// The interpretation state of one typed source value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceValueState {
    /// A value is present.
    Present,
    /// The field is absent.
    Absent,
    /// The source explicitly stores null.
    Null,
    /// The source value cannot be read.
    Unreadable,
    /// The concrete source shape is unsupported.
    Unsupported,
    /// The typed value is invalid.
    Invalid,
}

/// A typed data value with its immediate, materialized source origin.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "SourcedValueOf{T}", transform = source_value_schema::correlate)]
pub struct SourcedValue<T> {
    state: SourceValueState,
    #[schemars(with = "T")]
    value: Option<T>,
    #[serde(flatten)]
    origin: FieldOrigin,
}

impl<T> SourcedValue<T> {
    pub(crate) fn new(value: SourceValue<T>, origin: FieldOrigin) -> Self {
        let (state, value) = match value {
            SourceValue::Present(value) => (SourceValueState::Present, Some(value)),
            SourceValue::Absent => (SourceValueState::Absent, None),
            SourceValue::Null => (SourceValueState::Null, None),
            SourceValue::Unreadable => (SourceValueState::Unreadable, None),
            SourceValue::Unsupported => (SourceValueState::Unsupported, None),
            SourceValue::Invalid => (SourceValueState::Invalid, None),
        };
        Self {
            state,
            value,
            origin,
        }
    }

    /// Returns the captured source state independently of value truthiness.
    #[must_use]
    pub const fn state(&self) -> SourceValueState {
        self.state
    }

    /// Borrows the present value without collapsing false, zero or an empty collection.
    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the immediate contribution set and encryption filter index.
    #[must_use]
    pub const fn origin(&self) -> &FieldOrigin {
        &self.origin
    }

    /// Maps only a present value and retains its state and shared contribution set.
    #[must_use]
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> SourcedValue<U> {
        SourcedValue {
            state: self.state,
            value: self.value.map(map),
            origin: self.origin,
        }
    }

    /// Maps a present value through a fallible projection and retains its source state.
    ///
    /// # Errors
    /// Returns the projection error without constructing a replacement value.
    pub fn try_map<U, E>(self, map: impl FnOnce(T) -> Result<U, E>) -> Result<SourcedValue<U>, E> {
        Ok(SourcedValue {
            state: self.state,
            value: self.value.map(map).transpose()?,
            origin: self.origin,
        })
    }
}
