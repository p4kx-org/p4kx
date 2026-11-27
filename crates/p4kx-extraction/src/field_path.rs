// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

/// An exact escaped JSON Pointer into one complete output record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("pattern" = "^(?:/(?:[^~]|~[01])*)?$"))]
pub struct FieldPath(String);

/// A field address cannot be represented by the JSON Pointer contract.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FieldPathError {
    /// The pointer contains an invalid prefix or escape.
    #[error("field address is not an escaped JSON Pointer")]
    InvalidPointer,
    /// The pointer length cannot be represented.
    #[error("field address length cannot be represented")]
    TooLong,
    /// Allocation of the address failed.
    #[error("field address allocation failed")]
    Allocation(#[source] std::collections::TryReserveError),
}

impl FieldPath {
    /// Addresses one explicitly opaque whole value.
    #[must_use]
    pub const fn root() -> Self {
        Self(String::new())
    }

    /// Validates and owns an exact escaped pointer without interpreting its data.
    ///
    /// # Errors
    /// Returns invalid pointer syntax or an allocation error.
    pub fn parse(pointer: &str) -> Result<Self, FieldPathError> {
        validate(pointer)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(pointer.len())
            .map_err(FieldPathError::Allocation)?;
        owned.push_str(pointer);
        Ok(Self(owned))
    }

    /// Appends one original object field, escaping slash and tilde.
    ///
    /// # Errors
    /// Returns a length overflow or allocation error without changing the pointer.
    pub fn push_field(&mut self, field: &str) -> Result<(), FieldPathError> {
        let extra = field
            .bytes()
            .try_fold(1_usize, |length, byte| {
                length.checked_add(if matches!(byte, b'/' | b'~') { 2 } else { 1 })
            })
            .ok_or(FieldPathError::TooLong)?;
        self.reserve(extra)?;
        self.0.push('/');
        for character in field.chars() {
            match character {
                '/' => self.0.push_str("~1"),
                '~' => self.0.push_str("~0"),
                other => self.0.push(other),
            }
        }
        Ok(())
    }

    /// Appends an original zero-based array position.
    ///
    /// # Errors
    /// Returns a length overflow or allocation error without changing the pointer.
    pub fn push_index(&mut self, index: usize) -> Result<(), FieldPathError> {
        use std::fmt::Write as _;
        let mut digits = 1;
        let mut remaining = index;
        while remaining >= 10 {
            digits += 1;
            remaining /= 10;
        }
        self.reserve(digits + 1)?;
        let _written = write!(&mut self.0, "/{index}");
        Ok(())
    }

    /// Removes the last addressed segment.
    pub fn pop(&mut self) {
        if let Some(position) = self.0.rfind('/') {
            self.0.truncate(position);
        }
    }

    /// Returns the escaped pointer into the complete record.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn reserve(&mut self, extra: usize) -> Result<(), FieldPathError> {
        self.0
            .len()
            .checked_add(extra)
            .ok_or(FieldPathError::TooLong)?;
        self.0
            .try_reserve(extra)
            .map_err(FieldPathError::Allocation)
    }
}

fn validate(pointer: &str) -> Result<(), FieldPathError> {
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err(FieldPathError::InvalidPointer);
    }
    let mut bytes = pointer.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return Err(FieldPathError::InvalidPointer);
        }
    }
    Ok(())
}
