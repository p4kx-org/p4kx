// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};

use super::Observed;
use crate::CatalogError;
use crate::source_fields::FieldError;

impl Observed<'_, '_, '_, '_> {
    pub(in crate::catalog_v3) fn uint8(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<u8>, CatalogError> {
        self.scalar(name, "a nullable UInt8", notes, |fact| {
            Ok(match fact.value() {
                Value::UInt8(value) => Some(*value),
                _ => None,
            })
        })
    }

    pub(in crate::catalog_v3) fn uint16(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<u16>, CatalogError> {
        self.scalar(name, "a nullable UInt16", notes, |fact| {
            Ok(match fact.value() {
                Value::UInt16(value) => Some(*value),
                _ => None,
            })
        })
    }

    pub(in crate::catalog_v3) fn uint32(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        self.scalar(name, "a nullable UInt32", notes, |fact| {
            Ok(match fact.value() {
                Value::UInt32(value) => Some(*value),
                _ => None,
            })
        })
    }
}
