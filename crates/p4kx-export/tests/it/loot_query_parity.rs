// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog_from;
use p4kx_export::catalog_queries::loot::{LootDefinitionKind, LootDefinitionSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{Fixture, Input, TestResult, options};
use crate::sdk_full_proof as full_proof;
use crate::{loot_fixture, starmap_fixture};

#[test]
fn loot_sdk_queries_physical_owners_and_their_same_catalog_views() -> TestResult {
    let source = loot_fixture::database_with_loot_aliases().build();
    let fixture = loot_fixture::source_fixture(&source, Input::Encrypted)?;
    let built = build_catalog_from(
        &fixture.archive,
        &fixture.entry,
        options().with_full_provenance(),
    )?;
    let catalog = serde_json::from_slice(built.json())?;
    let session = full_proof::from_entry(
        &fixture.archive,
        &fixture.entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let all = compare_tables(&parsed, &catalog, CatalogRecordSelection::All)?;
    let guid = starmap_fixture::guid_text(loot_fixture::TABLE).parse()?;
    let by_guid = compare_tables(&parsed, &catalog, CatalogRecordSelection::Guid(guid))?;
    assert_eq!(
        all, by_guid,
        "GUID selection preserves both admitted aliases"
    );
    assert_eq!(
        all.len(),
        2,
        "the equal GUID has two admitted physical owners"
    );
    for index in all {
        let rows = compare_tables(
            &parsed,
            &catalog,
            CatalogRecordSelection::RecordIndex(index),
        )?;
        assert_eq!(
            rows,
            [index],
            "physical selection returns one original owner"
        );
    }
    assert_outside(&fixture, &parsed)?;
    assert_archetype(&fixture, &catalog)
}

fn compare_tables(
    parsed: &ParsedCatalogQueries<'_>,
    catalog: &Value,
    selection: CatalogRecordSelection,
) -> TestResult<Vec<u32>> {
    parsed.with_loot_definitions(LootDefinitionKind::Table, selection, None, |snapshot| {
        if matches!(selection, CatalogRecordSelection::All) {
            assert_eq!(
                snapshot.coverage.skipped_leftovers, 1,
                "the same debug owner is skipped"
            );
        }
        compare(
            &snapshot,
            catalog,
            "/definitions/lootTables/rows",
            "/lootTables",
        )
    })?
}

fn assert_outside(fixture: &Fixture, parsed: &ParsedCatalogQueries<'_>) -> TestResult {
    let forge = fixture.entry.open()?;
    let outside = forge
        .records()
        .find(|record| record.record_path().ok() == Some("unselected/loottables/outside"))
        .ok_or("physical outside fixture owner")?;
    parsed.with_loot_definitions(
        LootDefinitionKind::Table,
        CatalogRecordSelection::RecordIndex(outside.index()),
        None,
        |snapshot| {
            assert!(
                snapshot.rows.is_empty(),
                "physical selection still applies path admission"
            );
            assert!(
                snapshot.tables.is_empty(),
                "an unadmitted owner has no projected view"
            );
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "owner selection does not determine declaration availability"
            );
        },
    )?;
    Ok(())
}

fn assert_archetype(fixture: &Fixture, catalog: &Value) -> TestResult {
    let forge = fixture.entry.open()?;
    let record = forge
        .records()
        .find(|record| record.type_name().ok() == Some("LootArchetypeV3Record"))
        .ok_or("physical archetype owner")?;
    full_proof::from_entry(
        &fixture.archive,
        &fixture.entry,
        full_proof::NO_LANGUAGES,
        None,
    )?
    .with_loot_definitions(
        LootDefinitionKind::Archetype,
        CatalogRecordSelection::RecordIndex(record.index()),
        None,
        |snapshot| {
            let indices = compare(
                &snapshot,
                catalog,
                "/definitions/lootArchetypes/rows",
                "/lootArchetypes",
            )?;
            assert_eq!(
                indices,
                [record.index()],
                "archetype uses the same physical selector"
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )?
}

fn compare(
    snapshot: &LootDefinitionSnapshot<'_>,
    catalog: &Value,
    definition_path: &str,
    view_path: &str,
) -> TestResult<Vec<u32>> {
    let provenance = json!({"provenance": snapshot.source_inventory});
    let expected = proof::rows(catalog, definition_path)?;
    let views = proof::rows(catalog, view_path)?;
    let table = view_path == "/lootTables";
    let mut indices = Vec::new();
    for (position, row) in snapshot.rows.iter().enumerate() {
        let index = *row
            .source()
            .record_index()
            .value()
            .ok_or("physical Record index")?;
        indices.push(index);
        let matching = find(expected, "/recordIndex/value", index)?;
        assert_eq!(
            proof::with_origins(serde_json::to_value(row)?, &provenance)?,
            proof::with_origins(matching.clone(), catalog)?,
            "the selective original values match the Catalog including physical provenance"
        );
        let view = if table {
            serde_json::to_value(snapshot.tables.get(position).ok_or("weighted table view")?)?
        } else {
            serde_json::to_value(
                snapshot
                    .archetypes
                    .get(position)
                    .ok_or("weighted archetype view")?,
            )?
        };
        assert_eq!(
            proof::with_origins(view, &provenance)?,
            proof::with_origins(find(views, "/recordIndex", index)?.clone(), catalog)?,
            "the selective weighted view matches the same Catalog physical owner"
        );
    }
    Ok(indices)
}

fn find<'a>(rows: &'a [Value], index_path: &str, index: u32) -> TestResult<&'a Value> {
    rows.iter()
        .find(|row| row.pointer(index_path).and_then(Value::as_u64) == Some(u64::from(index)))
        .ok_or_else(|| "the same physical owner must occur in the Catalog".into())
}
