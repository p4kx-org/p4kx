// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups read without a catalog build agree with the `variantOf` of the catalog.

use crate::variants_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, Guid, build_catalog, read_variant_groups};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn guid(value: &Value) -> Result<Guid, Box<dyn Error>> {
    Ok(value.as_str().ok_or("not a GUID")?.parse()?)
}

#[test]
fn the_groups_are_the_variant_of_of_every_ship_and_item_of_the_catalog() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        variants_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(
        &archive,
        &CatalogOptions {
            publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        },
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;

    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;

    let mut checked = 0;
    for kind in ["ships", "items"] {
        for entity in catalog[kind].as_array().ok_or("no entities")? {
            let base = match &entity["variantOf"] {
                Value::Null => None,
                value => Some(guid(value)?),
            };
            let own = guid(&entity["guid"])?;
            assert_eq!(groups.base_of(own), base, "{}", entity["className"]);
            if let Some(base) = base {
                assert!(groups.variants_of(base).contains(&own), "listed");
            }
            checked += 1;
        }
    }
    assert!(checked > 30, "{checked} entities compared");
    assert!(!groups.is_empty());
    assert_eq!(groups.summary(), built.summary().variants());
    Ok(())
}

#[test]
fn a_stop_ends_the_read_without_groups() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        variants_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;

    assert!(read_variant_groups(&archive, &forge, &|| true).is_none());
    Ok(())
}
