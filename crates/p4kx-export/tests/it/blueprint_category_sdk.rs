// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogRecordSelection, read_blueprint_categories};
use p4kx_export::{Filter, build_catalog_from};
use p4kx_extraction::DataForgeEntry;
use serde_json::Value;

use crate::blueprint_category_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

use crate::sdk_full_proof as full_proof;

const OUTSIDE: &str = "libs/foundry/records/crafting/blueprintcategories/outside/other.xml";
const LEFTOVER: &str = "BlueprintCategoryRecord.Test_Leftover";

#[test]
fn category_query_matches_the_same_filtered_catalog_row() -> TestResult {
    let data = fixture::single().record(fixture::category(fixture::LOWER, "Ordinary", OUTSIDE));
    let (_folder, archive) = common::open(&data, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let filter = Filter::new("*/included/*");
    let mut options = common::options();
    options.filter = Some(&filter);
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, options.with_full_provenance())?.json(),
    )?;
    let filtered = fixture::rows(&catalog)?;
    assert_eq!(
        filtered.len(),
        1,
        "the path filter excludes the other category"
    );

    let session = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    let selected =
        session.read_blueprint_categories(CatalogRecordSelection::RecordIndex(0), None)?;
    let row = selected.rows.first().ok_or("selected physical category")?;
    assert_eq!(selected.rows.len(), 1, "one exact physical owner");
    assert_eq!(
        serde_json::to_value(row)?,
        filtered[0],
        "same row including origin IDs"
    );
    assert_eq!(
        selected.source_inventory.source_count(),
        1,
        "only the original DataForge contributes"
    );
    let origin = row.source.guid.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "authored DCB is encrypted"
    );
    assert!(
        !origin.source_uses_refs().is_empty()
            && origin.source_uses_refs().iter().all(|reference| {
                selected
                    .source_inventory
                    .iter_uses()
                    .any(|used| used.id() == reference)
            }),
        "the standalone query inventory resolves the category GUID's origins"
    );
    Ok(())
}

#[test]
fn category_queries_keep_duplicate_guids_and_exclude_leftover_records() -> TestResult {
    let data = fixture::single()
        .record(fixture::category(fixture::CATEGORY, "Duplicate", OUTSIDE))
        .record(fixture::category(fixture::LEFTOVER, LEFTOVER, OUTSIDE));
    let (_folder, archive) = common::open(&data, DcbOrigin::Plain)?;
    let all = read_blueprint_categories(&archive, CatalogRecordSelection::All, None)?;
    assert_eq!(
        all.coverage.skipped_leftovers, 1,
        "canonical leftover policy"
    );
    let source_guid = *all
        .rows
        .first()
        .and_then(|row| row.source.guid.value())
        .ok_or("original GUID")?;
    let selected =
        read_blueprint_categories(&archive, CatalogRecordSelection::Guid(source_guid), None)?;
    let indices: Vec<_> = selected
        .rows
        .iter()
        .filter_map(|row| row.source.record_index.value().copied())
        .collect();
    assert_eq!(
        indices,
        [0, 1],
        "a GUID selects all admitted physical owners in definition order"
    );
    let excluded =
        read_blueprint_categories(&archive, CatalogRecordSelection::RecordIndex(2), None)?;
    assert!(excluded.rows.is_empty());
    assert_eq!(excluded.coverage.skipped_leftovers, 1);
    Ok(())
}
