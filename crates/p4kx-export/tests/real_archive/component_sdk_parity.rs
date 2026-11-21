// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_extraction::{Archive, DataForgeEntry};
use serde::Serialize;
use serde_json::{Value, json};

use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_item_components::component_rows;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) struct ComponentSdkParity<'source> {
    archive: &'source Archive,
    entry: &'source DataForgeEntry,
    catalog: &'source Value,
    origins: Origins<'source>,
}

impl<'source> ComponentSdkParity<'source> {
    pub(super) fn new(
        archive: &'source Archive,
        entry: &'source DataForgeEntry,
        catalog: &'source Value,
    ) -> TestResult<Self> {
        Ok(Self {
            archive,
            entry,
            catalog,
            origins: Origins::read(catalog, entry.origin())?,
        })
    }

    pub(super) fn compare<Row: Serialize>(
        &self,
        dataset: &str,
        owner: u32,
        sdk: &EquipmentSnapshot<'_, Row>,
    ) -> TestResult {
        let catalog_rows = component_rows(self.catalog, dataset)?;
        let expected: Vec<_> = catalog_rows
            .iter()
            .filter_map(|((index, _), row)| (*index == owner).then_some(*row))
            .collect();
        if expected.is_empty()
            || sdk.build() != self.archive.build()
            || !matches!(sdk.availability(), DatasetAvailability::Available)
            || !sdk.coverage().unreadable_headers.is_empty()
            || !sdk.coverage().unreadable_bodies.is_empty()
            || sdk.rows().len() != expected.len()
        {
            return Err(format!("SDK build or physical owner scope differs for {dataset}").into());
        }
        let document = json!({"provenance": sdk.source_inventory()});
        let sdk_origins = Origins::read(&document, self.entry.origin())?;
        let actual: Vec<Value> = serde_json::from_slice(&serde_json::to_vec(sdk.rows())?)?;
        for (selected, produced) in actual.iter().zip(expected) {
            if sdk_origins.with_resolved_contributors(selected)?
                != self.origins.with_resolved_contributors(produced)?
            {
                return Err(
                    format!("SDK row or original contributors differ for {dataset}").into(),
                );
            }
        }
        Ok(())
    }
}
