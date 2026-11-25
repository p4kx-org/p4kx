// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::with_ship_store_slugs_from;
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{Archive, DataForgeEntry, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
use serde_json::{Value, json};

use super::super::super::SHIP_STORE_SLUGS;
use super::super::{Route, TestResult};

const ACTIVE: &str = "Aurora-Mk-I-ES";
const FAILED: &str = "Aurora-Mk-I-LX";
const ALIAS_PATH: &str = "alias_ship.xml";

#[derive(Clone, Copy)]
enum SourceFixture {
    Readable,
    UnreadableShips,
}

pub(super) fn read() -> TestResult<Route> {
    check_unreadable_sources()?;
    let (_dir, archive) = open(SourceFixture::Readable)?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let (_, route) = verify_rows(&archive, &entry, &options, &[], |scope| {
        let [row] = scope.rows() else {
            return Err("one successfully resolved curated Ship remains".into());
        };
        assert_eq!(
            row.slug(),
            ACTIVE,
            "the physical alias cannot hide the matching class"
        );
        assert!(
            row.ship_guid().is_some(),
            "the active Store row retains its canonical GUID"
        );
        assert!(!row.is_retired(), "the fixture proves an active mapping");
        Route::queried_rows(SHIP_STORE_SLUGS, scope.rows(), None)
    })?;
    Ok(route)
}

fn check_unreadable_sources() -> TestResult<()> {
    let (_dir, archive) = open(SourceFixture::UnreadableShips)?;
    let entry = DataForgeEntry::read(&archive)?;
    let active = Filter::new("mapped_ship");
    let alias = Filter::new(ALIAS_PATH.trim_end_matches(".xml"));
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    for filter in [&active, &alias] {
        options.filter = Some(filter);
        let (rows, ()) = verify_rows(&archive, &entry, &options, &[], |_| Ok(()))?;
        assert_eq!(
            rows,
            usize::from(filter == &active),
            "a matching physical GUID needs its exact curated class in the same Filter"
        );
    }
    options.filter = None;
    assert!(
        matches!(
            p4kx_export::build_catalog_from(&archive, &entry, options.with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the unfiltered Normal export rejects an unreadable required Ship"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let (rows, ()) = verify_rows(&archive, &entry, &options, &[2], |_| Ok(()))?;
    assert_eq!(
        rows, 1,
        "the degraded read preserves the healthy mapped Ship"
    );
    Ok(())
}

fn verify_rows<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: &CatalogOptions<'_>,
    unreadable: &[u32],
    inspect: impl FnOnce(
        p4kx_export::catalog_queries::equipment::ShipStoreSlugSnapshot<'_>,
    ) -> TestResult<T>,
) -> TestResult<(usize, T)> {
    let built = p4kx_export::build_catalog_from(archive, entry, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    if options.filter.is_none()
        && matches!(options.publication_mode, CatalogPublicationMode::Normal)
    {
        for (field, expected) in [
            ("publicationMode", "normal"),
            ("status", "complete"),
            ("read", "complete"),
            ("projection", "complete"),
        ] {
            assert_eq!(
                catalog
                    .pointer(&format!("/exportHealth/{field}"))
                    .and_then(Value::as_str),
                Some(expected),
                "the unfiltered readable fixture must publish a complete Normal Catalog"
            );
        }
    }
    with_ship_store_slugs_from(archive, entry, options.filter, None, |scope| {
        let rows = scope.rows();
        assert_eq!(
            serde_json::to_value(rows)?,
            *catalog
                .pointer(SHIP_STORE_SLUGS)
                .ok_or("the canonical Store rows")?,
            "SDK rows share canonical filtered and unfiltered resolution"
        );
        let filter = scope.filter().map_or_else(
            || json!({"status": "complete"}),
            |filter| json!({"status": "partial", "filter": filter}),
        );
        assert_eq!(
            filter,
            *catalog
                .pointer("/completeness/shipStoreSlugs")
                .ok_or("Store completeness")?,
            "the SDK reports the exact producer selection"
        );
        assert_eq!(
            scope.summary(),
            built.summary().store_slugs(),
            "the exact selection preserves curated counts and stale decisions"
        );
        assert_eq!(
            scope.coverage().unreadable_bodies.as_slice(),
            unreadable,
            "only a demanded, selected failing Ship affects direct coverage"
        );
        let count = rows.len();
        inspect(scope).map(|result| (count, result))
    })?
}

fn open(sources: SourceFixture) -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("curated-slug-scope.p4k");
    let curated: Value = serde_json::from_str(include_str!("../../data/ship-store-slugs.json"))?;
    let (guid, class) = curated_ship(&curated, ACTIVE)?;
    let mut database = entity_schema(DataForgeBuilder::new().version(8))
        .record(ship(guid, "UnmappedAlias", ALIAS_PATH).record())
        .record(ship(guid, class, "mapped_ship.xml").record());
    if matches!(sources, SourceFixture::UnreadableShips) {
        let (broken_guid, broken_class) = curated_ship(&curated, FAILED)?;
        database = database
            .record(
                ship(broken_guid, broken_class, "failed_ship.xml")
                    .record()
                    .with_instance_index(u16::MAX),
            )
            .record(
                ship(0x01, "BrokenNeighbor", "unrelated_ship.xml")
                    .record()
                    .with_instance_index(u16::MAX),
            );
    }
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database.build()))
        .write_to(&path)?;
    Ok((dir, Archive::open(path)?))
}

fn curated_ship<'source>(source: &'source Value, slug: &str) -> TestResult<(u128, &'source str)> {
    let row = source
        .get("ships")
        .and_then(Value::as_array)
        .and_then(|ships| {
            ships
                .iter()
                .find(|row| row.get("slug").and_then(Value::as_str) == Some(slug))
        })
        .ok_or("the fixture uses an embedded mapped Store ship")?;
    let guid = row
        .get("guid")
        .and_then(Value::as_str)
        .ok_or("the curated original GUID")?;
    let class = row
        .get("className")
        .and_then(Value::as_str)
        .ok_or("the curated original class")?;
    Ok((u128::from_str_radix(&guid.replace('-', ""), 16)?, class))
}

fn ship(guid: u128, class: &str, file: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!("libs/foundry/records/entities/spaceships/{file}"),
    )
}
