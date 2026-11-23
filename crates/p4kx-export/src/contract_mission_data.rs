// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::Kind;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::GameTextRef;
use crate::contract::every_field_required;

/// A mission type: a `MissionType` Record under `missiontype`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionType<'a> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'a>,
    /// The localised name of the type (`LocalisedTypeName`).
    #[schemars(extend("x-gameText" = true))]
    pub(crate) type_name: SourcedValue<GameTextRef>,
}

impl<'source> MissionType<'source> {
    /// The physical identity and origin of the mission type.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// The authored localized name reference.
    #[must_use]
    pub const fn type_name(&self) -> &SourcedValue<GameTextRef> {
        &self.type_name
    }

    pub(crate) fn table(forge: &DataForge<'_>, mut rows: Vec<Self>) -> CatalogTable<Self> {
        rows.sort_by_cached_key(|row| {
            (
                row.source.guid.value().map(|guid| guid.text()),
                row.source.record_index.value().copied(),
            )
        });
        CatalogTable::new(forge, &[Kind::MissionTypes.records().type_name], rows)
    }
}
