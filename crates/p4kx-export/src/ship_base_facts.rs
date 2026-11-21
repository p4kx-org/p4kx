// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::ship_base::ShipCargoSource;

/// Interpreted base facts for one physical Ship Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipBaseFacts {
    ship_guid: SourcedValue<CatalogGuid>,
    ship_record_index: SourcedValue<u32>,
    cargo: ShipCargoSource,
}

impl ShipBaseFacts {
    pub(crate) fn new(header: &CanonicalRecordHeader<'_>, cargo: ShipCargoSource) -> Self {
        Self {
            ship_guid: header.guid.clone(),
            ship_record_index: header.record_index().clone(),
            cargo,
        }
    }

    /// Returns the original Ship GUID with its source evidence.
    #[must_use]
    pub fn ship_guid(&self) -> SourcedValue<Guid> {
        self.ship_guid.map_ref(|guid| guid.0)
    }

    /// Borrows the physical Ship Record index with its source evidence.
    #[must_use]
    pub const fn ship_record_index(&self) -> &SourcedValue<u32> {
        &self.ship_record_index
    }

    /// Borrows the resolved cargo values and their contributing mounts.
    #[must_use]
    pub const fn cargo(&self) -> &ShipCargoSource {
        &self.cargo
    }
}

pub(crate) fn table(
    forge: &DataForge<'_>,
    mut rows: Vec<ShipBaseFacts>,
) -> CatalogTable<ShipBaseFacts> {
    rows.sort_by_cached_key(|row| {
        (
            row.ship_guid.value().map(|guid| guid.text()),
            row.ship_record_index.value().copied(),
        )
    });
    CatalogTable::new(forge, &[p4kx_assembly::ENTITY_TYPE], rows)
}
