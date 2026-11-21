// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::authored_fields::Observed;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// One physical refining process with its original labels and complete field arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct RefiningProcessDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    fields: SourceArenaOutput<'source>,
    #[schemars(extend("x-gameText" = true))]
    process_name: SourcedValue<GameTextRef>,
    refining_speed: SourcedValue<&'source str>,
    refining_quality: SourcedValue<&'source str>,
}

impl<'source> RefiningProcessDefinition<'source> {
    /// Borrows the independently sourced physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }
    /// Borrows every original field and occurrence of this physical owner.
    #[must_use]
    pub const fn fields(&self) -> &SourceArenaOutput<'source> {
        &self.fields
    }
    /// Borrows the protected original process-name text reference.
    #[must_use]
    pub const fn process_name(&self) -> &SourcedValue<GameTextRef> {
        &self.process_name
    }
    /// Borrows the exact open speed label and source state.
    #[must_use]
    pub const fn refining_speed(&self) -> &SourcedValue<&'source str> {
        &self.refining_speed
    }
    /// Borrows the exact open quality label and source state.
    #[must_use]
    pub const fn refining_quality(&self) -> &SourcedValue<&'source str> {
        &self.refining_quality
    }

    pub(super) fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source,
            fields: fields.authored_members()?,
            process_name: fields.protected_locale("processName", projection, notes)?,
            refining_speed: label(fields, "refiningSpeed", notes)?,
            refining_quality: label(fields, "refiningQuality", notes)?,
        })
    }
}

fn label<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<&'source str>, CatalogError> {
    match fields.storage_type(name)? {
        Some(DataType::String) => fields.borrowed_string(name, notes),
        _ => fields.borrowed_declared_enum(name, notes),
    }
}
