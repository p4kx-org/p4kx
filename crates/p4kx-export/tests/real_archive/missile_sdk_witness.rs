// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde::Serialize;
use serde_json::{Value, json};

use super::{ROWS, TestResult, components};
use crate::real_archive_auto_spawns::provenance::Origins;

const EXPLOSIVES: &str = "/configurations/explosives/rows";

pub(super) fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    catalog: &Value,
    owner: u32,
) -> TestResult {
    let origins = Origins::read(catalog, entry.origin())?;
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "admit the same installed Archive for the ordnance SDK")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "parse the same DataForge for the ordnance SDK")?;
    let selection = CatalogRecordSelection::RecordIndex(owner);
    parsed
        .with_missile_configurations(selection, None, |snapshot| {
            compare_rows(archive, entry, catalog, &origins, ROWS, owner, &snapshot)
        })
        .map_err(|_| "query missile configuration for the installed owner")??;
    parsed
        .with_explosives(selection, None, |snapshot| {
            compare_rows(
                archive, entry, catalog, &origins, EXPLOSIVES, owner, &snapshot,
            )
        })
        .map_err(|_| "query explosive configuration for the installed owner")??;
    Ok(())
}

fn compare_rows<Row: Serialize>(
    archive: &Archive,
    entry: &DataForgeEntry,
    catalog: &Value,
    catalog_origins: &Origins<'_>,
    dataset: &str,
    owner: u32,
    sdk: &EquipmentSnapshot<'_, Row>,
) -> TestResult {
    let catalog_rows = components::component_rows(catalog, dataset)?;
    let expected: Vec<_> = catalog_rows
        .iter()
        .filter_map(|((index, _), row)| (*index == owner).then_some(*row))
        .collect();
    if expected.is_empty()
        || sdk.build() != archive.build()
        || !matches!(sdk.availability(), DatasetAvailability::Available)
        || !sdk.coverage().unreadable_headers.is_empty()
        || !sdk.coverage().unreadable_bodies.is_empty()
        || sdk.rows().len() != expected.len()
    {
        return Err(format!("SDK build or physical owner scope differs for {dataset}").into());
    }
    let document = json!({"provenance": sdk.source_inventory()});
    let sdk_origins = Origins::read(&document, entry.origin())?;
    let actual: Vec<Value> = serde_json::from_slice(&serde_json::to_vec(sdk.rows())?)?;
    for (selected, produced) in actual.iter().zip(expected) {
        if sdk_origins.with_resolved_contributors(selected)?
            != catalog_origins.with_resolved_contributors(produced)?
        {
            return Err(format!("SDK row or original contributors differ for {dataset}").into());
        }
    }
    Ok(())
}
