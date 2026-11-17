// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

use super::data_type::SourceDataType;
use super::value::SourceValueRef;

/// One original field occurrence with its exact name and decode declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SourceField<'source> {
    pub(crate) name: SourcedValue<&'source str>,
    pub(crate) declaration: SourceDeclaration,
    pub(crate) value: SourceValueRef,
}

impl<'source> SourceField<'source> {
    /// Borrows the original field name with its source evidence.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<&'source str> {
        &self.name
    }

    /// Borrows the original decoder declaration and its evidence.
    #[must_use]
    pub const fn declaration(&self) -> &SourceDeclaration {
        &self.declaration
    }

    /// Returns the occurrence's link within its owner's value arena.
    #[must_use]
    pub const fn value(&self) -> SourceValueRef {
        self.value
    }
}

/// The actual original decoder storage of a field and its elements.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct SourceDeclaration {
    pub(crate) data_type: SourcedValue<SourceDataType>,
    pub(crate) is_array: SourcedValue<bool>,
    pub(crate) target: SourcedValue<u16>,
}

impl SourceDeclaration {
    /// Borrows the original element storage kind.
    #[must_use]
    pub const fn data_type(&self) -> &SourcedValue<SourceDataType> {
        &self.data_type
    }

    /// Borrows the original array declaration flag.
    #[must_use]
    pub const fn is_array(&self) -> &SourcedValue<bool> {
        &self.is_array
    }

    /// Borrows the original target structure index.
    #[must_use]
    pub const fn target(&self) -> &SourcedValue<u16> {
        &self.target
    }
}
