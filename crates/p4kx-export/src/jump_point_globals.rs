// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, DataForge, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::referenced_definition_read::{self, Definitions, Selection};
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

const POINT: &str = "GlobalJumpPointParams";
const TUNNEL: &str = "GlobalJumpTunnelHostParams";
pub(crate) const SOURCE_TYPES: &[&str] = &[POINT, TUNNEL];

/// An original global jump configuration and its authored field graph.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct GlobalJumpConfiguration<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    fields: SourceArenaOutput<'source>,
}

impl<'source> GlobalJumpConfiguration<'source> {
    /// Borrows the original global configuration Record identity.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the authored fields in original order.
    #[must_use]
    pub const fn fields(&self) -> &SourceArenaOutput<'source> {
        &self.fields
    }

    pub(in crate::catalog_v3) fn read(
        _kind: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        _notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source,
            fields: fields.authored_members()?,
        })
    }
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Definitions<'source, GlobalJumpConfiguration<'source>>, CatalogError> {
    referenced_definition_read::read(
        forge,
        input,
        &Selection {
            source_types: SOURCE_TYPES,
            linked: &BTreeSet::new(),
            filter,
            inspection: ConfigurationInspection::JumpPointGlobal,
        },
        cancel,
        GlobalJumpConfiguration::read,
    )
}

pub(super) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<(&'source str, GlobalJumpConfiguration<'source>)>,
) -> CatalogTable<GlobalJumpConfiguration<'source>> {
    CatalogTable::new(
        forge,
        SOURCE_TYPES,
        rows.into_iter().map(|(_, row)| row).collect(),
    )
}
