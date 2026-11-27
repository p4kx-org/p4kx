// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::{FieldOrigin, FieldPath};

/// Immediate per-value origins addressed inside one complete unchanged output record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct FieldOrigins {
    entries: BTreeMap<FieldPath, FieldOrigin>,
}

/// A field-origin companion cannot be represented without ambiguous ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FieldOriginsError {
    /// An output field was assigned conflicting origin ownership.
    #[error("field origin is already present")]
    DuplicateField,
}

impl FieldOrigins {
    /// Starts an empty field-addressed companion without asserting source coverage.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Adds one already-derived shared origin without replacing existing field ownership.
    ///
    /// # Errors
    /// Returns an error when the same field already has an origin owner.
    pub fn insert(
        &mut self,
        path: FieldPath,
        origin: FieldOrigin,
    ) -> Result<(), FieldOriginsError> {
        if self.entries.contains_key(&path) {
            return Err(FieldOriginsError::DuplicateField);
        }
        self.entries.insert(path, origin);
        Ok(())
    }

    /// Returns the immediate origin at an exact field path.
    #[must_use]
    pub fn get(&self, path: &FieldPath) -> Option<&FieldOrigin> {
        self.entries.get(path)
    }

    /// Iterates stable escaped field paths and their already-materialized shared origins.
    pub fn iter(&self) -> impl Iterator<Item = (&FieldPath, &FieldOrigin)> {
        self.entries.iter()
    }
}
