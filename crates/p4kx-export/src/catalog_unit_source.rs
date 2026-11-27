// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceArenaOutput;

/// Concrete source type and the original members outside its typed fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct UnitSource<'source> {
    source_type: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> UnitSource<'source> {
    /// Borrows the original concrete unit type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows original members outside the typed unit fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    #[cfg(test)]
    pub(super) const fn fixture(source_type: SourcedValue<&'source str>) -> Self {
        Self {
            source_type,
            additional_fields: SourceArenaOutput {
                fields: Vec::new(),
                values: Vec::new(),
            },
        }
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projected: impl IntoIterator<Item = (&'static str, SourceValueState)>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: source.source_type_value()?,
            additional_fields: source.authored_members_except(projected)?,
        })
    }
}
