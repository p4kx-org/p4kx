// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::with_ship_store_slugs_from;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
use serde_json::{Value, json};

use super::super::super::SHIP_STORE_SLUGS;
use super::super::{Route, TestResult};

const ACTIVE: &str = "Aurora-Mk-I-ES";
const FAILED: &str = "Aurora-Mk-I-LX";
const ALIAS_PATH: &str = "alias_ship.xml";

pub(super) fn read() -> TestResult<Route> {
    let (_dir, archive) = open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let active = Filter::new("mapped_ship");
    let alias = Filter::new(ALIAS_PATH.trim_end_matches(".xml"));
    for filter in [&active, &alias] {
        let (rows, ()) = verify_rows(&archive, &entry, Some(filter), |_| Ok(()))?;
        assert_eq!(
            rows,
            usize::from(filter == &active),
            "a matching physical GUID needs its exact curated class in the same Filter"
        );
    }
    verify_rows(&archive, &entry, None, |scope| {
        let [row] = scope.rows() else {
            return Err("one successfully resolved curated Ship remains".into());
        };
        assert_eq!(row.slug(), ACTIVE, "the physical alias cannot hide the matching class");
        assert!(row.ship_guid().is_some(), "the active Store row retains its canonical GUID");
        assert!(!row.is_retired(), "the fixture proves an active mapping");
        Route::queried_rows(
            SHIP_STORE_SLUGS,
            scope.rows(),
            "full and Filter-scoped synthetic producer parity with physical aliases available; installed SDK parity open",
        )
    })
    .map(|(_, route)| route)
}

fn verify_rows<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    filter: Option<&Filter>,
    inspect: impl FnOnce(
        p4kx_export::catalog_queries::equipment::ShipStoreSlugSnapshot<'_>,
    ) -> TestResult<T>,
) -> TestResult<(usize, T)> {
    let options = CatalogOptions {
        filter,
        publication_mode: if filter.is_none() {
            CatalogPublicationMode::DiagnosticPartial
        } else {
            CatalogPublicationMode::Normal
        },
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    with_ship_store_slugs_from(archive, entry, filter, None, |scope| {
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
        let unreadable: &[u32] = if scope.filter().is_none() { &[2] } else { &[] };
        assert_eq!(
            scope.coverage().unreadable_bodies.as_slice(),
            unreadable,
            "only a demanded, selected failing Ship affects direct coverage"
        );
        let count = rows.len();
        inspect(scope).map(|result| (count, result))
    })?
}

fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("curated-slug-scope.p4k");
    let curated: Value = serde_json::from_str(include_str!("../../data/ship-store-slugs.json"))?;
    let (guid, class) = curated_ship(&curated, ACTIVE)?;
    let (broken_guid, broken_class) = curated_ship(&curated, FAILED)?;
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .record(ship(guid, "UnmappedAlias", ALIAS_PATH).record())
        .record(ship(guid, class, "mapped_ship.xml").record())
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
