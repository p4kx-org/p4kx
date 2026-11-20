// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::loot_generation::LootGenerationSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::{Value, json};

use super::{Origins, TestResult};

const OWNER: &str = "/ownerRecordIndex/value";

pub(super) fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    expected: &[Value],
    catalog_origins: &Origins<'_>,
) -> TestResult {
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "admit filtered Loot component SDK input")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "parse matching SDK DataForge")?;
    for owner_rows in expected.chunk_by(|left, right| left.pointer(OWNER) == right.pointer(OWNER)) {
        let index = owner_rows
            .first()
            .and_then(|row| row.pointer(OWNER))
            .and_then(Value::as_u64)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or("exact Catalog Loot owner index")?;
        parsed
            .with_loot_generation_components(
                CatalogRecordSelection::RecordIndex(index),
                None,
                |snapshot| compare_owner(archive, entry, owner_rows, catalog_origins, &snapshot),
            )
            .map_err(|_| "read original owner with the Loot component SDK")??;
    }
    Ok(())
}

fn compare_owner(
    archive: &Archive,
    entry: &DataForgeEntry,
    expected: &[Value],
    catalog_origins: &Origins<'_>,
    sdk: &LootGenerationSnapshot<'_>,
) -> TestResult {
    let coverage = sdk.coverage();
    if sdk.build() != archive.build()
        || !matches!(sdk.availability(), DatasetAvailability::Available)
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || sdk.rows().len() != expected.len()
    {
        return Err("SDK does not cover the same original Loot component owner".into());
    }
    let provenance = serde_json::to_value(sdk.source_inventory())
        .map_err(|_| "serialize the selective Loot source inventory")?;
    let document = json!({"provenance": provenance});
    let origins = Origins::read(&document, entry.origin())?;
    let bytes = serde_json::to_vec(sdk.rows())
        .map_err(|_| "write the complete selective Loot component rows")?;
    let actual: Vec<Value> = serde_json::from_slice(&bytes)
        .map_err(|_| "read the complete selective Loot component rows")?;
    for (catalog, sdk) in expected.iter().zip(&actual) {
        if catalog_origins.with_resolved_contributors(catalog)?
            != origins.with_resolved_contributors(sdk)?
        {
            return Err(
                "Loot component row or original contributor differs between SDK and Catalog".into(),
            );
        }
    }
    Ok(())
}
