// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

pub(super) const SCAN_VALUE_TYPES: &[&str] = &[
    "ScanCustomValue",
    "ScanCustomValueArray",
    "ScanCustomValueFloat",
    "ScanCustomValueLocString",
];

/// One original typed scan value with indices into the same `values` array.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum ScanValue {
    Base,
    Array {
        value: SourcedValue<Vec<SourcedValue<u32>>>,
    },
    Float {
        value: SourcedValue<ResourceFloat32>,
    },
    Locale {
        value: SourcedValue<GameTextRef>,
    },
}

#[derive(Default)]
pub(super) struct ValueQueue<'reader, 'scope, 'read, 'source> {
    pending: VecDeque<Observed<'reader, 'scope, 'read, 'source>>,
    next: u32,
}

impl<'reader, 'scope, 'read, 'source> ValueQueue<'reader, 'scope, 'read, 'source> {
    pub(super) fn push(
        &mut self,
        source: Observed<'reader, 'scope, 'read, 'source>,
    ) -> Result<u32, CatalogError> {
        let index = self.next;
        self.next = index
            .checked_add(1)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        self.pending.push_back(source);
        Ok(index)
    }

    pub(super) fn read(
        mut self,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Vec<ScanValue>, CatalogError> {
        let mut values = Vec::new();
        while let Some(source) = self.pending.pop_front() {
            source.check_cancel()?;
            let value = match source.source_type()? {
                "ScanCustomValue" => ScanValue::Base,
                "ScanCustomValueArray" => ScanValue::Array {
                    value: source.strong_pointer_array_to(
                        "value",
                        "ScanCustomValue",
                        SCAN_VALUE_TYPES,
                        notes,
                        |source, _| self.push(source),
                    )?,
                },
                "ScanCustomValueFloat" => ScanValue::Float {
                    value: source.single("value", notes)?,
                },
                "ScanCustomValueLocString" => ScanValue::Locale {
                    value: source.protected_locale("value", projection, notes)?,
                },
                _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
            };
            values.push(value);
        }
        Ok(values)
    }
}
