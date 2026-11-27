// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};

use crate::contract_store_slugs::ShipStoreSlug;

use super::{CatalogLanguageSelection, CatalogQuerySession, StoreSlugs};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn distinct_store_identities_resolve_to_one_physical_ship() -> TestResult {
    const MAPPING: &str = r#"{"format":1,"ships":[
        {"slug":"Active-Edition","rsiId":2,"storeName":"Active","className":"Active","guid":"00000000-0000-0000-0000-000000000001","retired":false},
        {"slug":"Active","rsiId":1,"storeName":"Active","className":"Active","guid":"00000000-0000-0000-0000-000000000001","retired":false}
    ]}"#;
    let mapping = StoreSlugs::parse(MAPPING)?;
    let (_dir, archive) = open()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    parsed.with_store_mapping(&mapping, None, None, |scope| -> TestResult {
        let [standard, edition] = scope.rows() else {
            return Err("one physical Ship must retain both Store identity rows".into());
        };
        assert_eq!(
            [
                (standard.rsi_id(), standard.slug()),
                (edition.rsi_id(), edition.slug())
            ],
            [(1, "Active"), (2, "Active-Edition")],
            "the two Store identities retain their independent keys in slug order"
        );
        assert_eq!(
            standard.ship_guid().map(|guid| guid.to_string()).as_deref(),
            Some("00000000-0000-0000-0000-000000000001"),
            "the source GUID of the physical fixture Ship resolves"
        );
        assert_eq!(
            (edition.ship_guid(), edition.class_name()),
            (standard.ship_guid(), Some("Active")),
            "the reverse GUID and class join includes both Store identities"
        );
        Ok(())
    })??;
    Ok(())
}

#[test]
fn retired_scope_keeps_missing_owners_and_checks_reappearing_guid_aliases() -> TestResult {
    const MAPPING: &str = r#"{"format":1,"ships":[
        {"slug":"Active","rsiId":1,"storeName":"Active","className":"Active","guid":"00000000-0000-0000-0000-000000000001","retired":false},
        {"slug":"Missing","rsiId":2,"storeName":"Missing","className":"Old","guid":"00000000-0000-0000-0000-000000000002","retired":true},
        {"slug":"Revived","rsiId":3,"storeName":"Revived","className":"Old","guid":"00000000-0000-0000-0000-000000000003","retired":true}
    ]}"#;
    let mapping = StoreSlugs::parse(MAPPING)?;
    let (_dir, archive) = open()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    parsed.with_store_mapping(&mapping, None, None, |scope| -> TestResult {
        let [active, retired] = scope.rows() else {
            return Err("both the active and absent retired mapping must be visible".into());
        };
        assert_eq!(active.slug(), "Active", "the active mapping resolves");
        assert_eq!(
            retired.slug(),
            "Missing",
            "the absent original GUID retains its retired row"
        );
        assert!(retired.is_retired(), "the retired flag is explicit");
        assert_eq!(
            retired.ship_guid(),
            None,
            "the retired row has no Ship target"
        );
        assert_eq!(
            retired.class_name(),
            None,
            "the retired row has no active class"
        );
        assert_eq!(
            scope.summary().stale,
            ["Revived"],
            "any admitted physical Ship with that GUID makes the retired mapping stale"
        );
        Ok(())
    })??;
    let filter = Filter::new("spaceships/");
    parsed.with_store_mapping(&mapping, Some(&filter), None, |scope| {
        assert_eq!(
            scope
                .rows()
                .iter()
                .map(ShipStoreSlug::slug)
                .collect::<Vec<_>>(),
            ["Active"],
            "a literal Filter keeps only resolving active mappings"
        );
        assert!(
            scope.summary().stale.is_empty(),
            "filtered selections make no stale assertion"
        );
    })?;
    Ok(())
}

fn open() -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("retired-slug-scope.p4k");
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(
                1,
                "Active",
                "libs/foundry/records/entities/spaceships/active.xml",
            )
            .record(),
        )
        .record(
            TestEntity::new(
                3,
                "RevivedAlias",
                "libs/foundry/records/entities/spaceships/revived.xml",
            )
            .record(),
        );
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database.build()))
        .write_to(&path)?;
    Ok((dir, Archive::open(path)?))
}
