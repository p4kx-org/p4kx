// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlueprintCategoryListSelection, BlueprintCategoryListSnapshot, read_blueprint_category_lists,
    read_blueprint_category_lists_from,
};
use p4kx_export::{CatalogError, build_catalog, build_catalog_from};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Guid, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use crate::category_lists_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn category_list_queries_match_the_catalog_and_close_encrypted_source_uses() -> TestResult {
    let (_folder, archive) = common::open(&fixture::clean(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let full = build_catalog_from(&archive, &entry, &common::options())?;
    let catalog: Value = serde_json::from_slice(full.json())?;
    let snapshot = read_blueprint_category_lists_from(
        &archive,
        &entry,
        BlueprintCategoryListSelection::All,
        None,
    )?;
    let expected = fixture::rows(&catalog)?;
    let projected = snapshot
        .rows
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        projected.as_slice(),
        expected,
        "all value and source-use fields match on the same DCB"
    );
    assert_eq!(
        snapshot.coverage.skipped_leftovers, 1,
        "the canonical leftover policy applies"
    );
    assert_eq!(
        snapshot.source_inventory.source_count(),
        1,
        "only the DCB was observed"
    );
    let (source_guid, position) = selected_record(&snapshot)?;
    for selection in [
        BlueprintCategoryListSelection::Guid(source_guid),
        BlueprintCategoryListSelection::RecordIndex(position),
    ] {
        let selected = read_blueprint_category_lists_from(&archive, &entry, selection, None)?;
        assert_eq!(
            selected.rows.len(),
            1,
            "one query result matches the selected identity"
        );
        assert_eq!(
            serde_json::to_value(&selected.rows[0])?,
            expected[0],
            "selection preserves row provenance"
        );
    }
    Ok(())
}

fn selected_record(
    snapshot: &BlueprintCategoryListSnapshot,
) -> Result<(Guid, u32), Box<dyn std::error::Error>> {
    let primary = snapshot.rows.first().ok_or("primary category list")?;
    let source_guid = *primary.source.guid.value().ok_or("sourced GUID")?;
    let position = *primary
        .source
        .record_index
        .value()
        .ok_or("sourced physical position")?;
    for origin in [primary.source.guid.origin(), primary.categories.origin()] {
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "same encrypted DCB"
        );
        assert!(
            !origin.source_uses_refs().is_empty(),
            "scope includes actual field uses"
        );
        for reference in origin.source_uses_refs() {
            assert!(
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|description| description.id() == reference),
                "each selected field resolves its original use"
            );
        }
    }
    Ok((source_guid, position))
}

#[test]
fn category_list_queries_retain_duplicate_guids_and_refuse_foreign_evidence() -> TestResult {
    let database = fixture::clean().record(fixture::listing(
        fixture::LIST,
        "Duplicate",
        "synthetic/second_physical_owner.xml",
        fixture::references(&[fixture::BETA]),
    ));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge.record(0).ok_or("reference Record")?.guid();
    let selected = read_blueprint_category_lists_from(
        &archive,
        &entry,
        BlueprintCategoryListSelection::Guid(guid),
        None,
    )?;
    let positions: Vec<_> = selected
        .rows
        .iter()
        .filter_map(|row| row.source.record_index.value().copied())
        .collect();
    assert_eq!(
        positions,
        [0, 6],
        "the GUID selection preserves both physical owners"
    );
    let (_other_folder, other) = common::open(&database, DcbOrigin::Plain)?;
    assert!(
        matches!(
            read_blueprint_category_lists_from(
                &other,
                &entry,
                BlueprintCategoryListSelection::All,
                None
            ),
            Err(CatalogError::SourceOriginMismatch)
        ),
        "equal bytes at another location cannot supply the selected Archive's evidence"
    );
    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(
        matches!(
            read_blueprint_category_lists(
                &archive,
                BlueprintCategoryListSelection::All,
                Some(&cancel)
            ),
            Err(CatalogError::Cancelled)
        ),
        "cancellation returns no snapshot"
    );
    Ok(())
}

#[test]
fn category_list_queries_are_independent_of_unreadable_localization_payloads() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::clean().build(),
        ))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"unused=Unrelated\n")
                .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::LoadTexts(_))
        ),
        "the unrelated localization is present and actually unreadable"
    );
    let selected = read_blueprint_category_lists(
        &archive,
        BlueprintCategoryListSelection::RecordIndex(0),
        None,
    )?;
    assert_eq!(
        selected.rows.len(),
        1,
        "the category declaration and its evidence still load"
    );
    assert_eq!(
        selected.source_inventory.source_count(),
        1,
        "only the DCB is in the dependency closure"
    );
    Ok(())
}
