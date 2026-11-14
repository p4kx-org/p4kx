// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
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

impl ScanValue {
    pub(super) fn read<'reader, 'scope, 'read, 'source>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "ScanCustomValue" => Self::Base,
            "ScanCustomValueArray" => Self::Array {
                value: source.strong_pointer_array_to(
                    "value",
                    "ScanCustomValue",
                    SCAN_VALUE_TYPES,
                    notes,
                    |source, _| pending.push(source),
                )?,
            },
            "ScanCustomValueFloat" => Self::Float {
                value: source.single("value", notes)?,
            },
            "ScanCustomValueLocString" => Self::Locale {
                value: source.protected_locale("value", projection, notes)?,
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}
