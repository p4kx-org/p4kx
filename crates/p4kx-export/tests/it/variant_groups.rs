// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups read without a catalog build agree with the `variantOf` of the catalog.

use crate::{item_stats_fixture, variants_fixture};

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

#[test]
fn shield_variant_groups_use_original_stats_after_the_wire_field_is_removed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        item_stats_fixture::write_shield_variant_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let items = catalog["items"].as_array().ok_or("catalog items")?;
    let item = |name: &str| {
        items
            .iter()
            .find(|item| item["className"] == name)
            .ok_or_else(|| format!("missing {name}"))
    };
    let base = item("shield_base")?;
    let twin = item("shield_base_twin")?;
    let different = item("shield_different")?;
    for row in [base, twin, different] {
        assert!(
            row["stats"].is_null(),
            "a shield with no remaining data has null stats"
        );
    }
    assert!(base["variantOf"].is_null(), "the base retains its identity");
    assert_eq!(
        twin["variantOf"], base["guid"],
        "the equal shield is grouped"
    );
    assert!(
        different["variantOf"].is_null(),
        "a shield with different authored health stays separate"
    );
    let resource_item = item("shield_with_resources")?;
    let stats = &resource_item["stats"];
    assert!(
        stats.get("shield").is_none(),
        "shield is removed from item stats"
    );
    assert!(
        stats["health"].is_null(),
        "the shield has resource states without health"
    );
    assert_eq!(
        stats["resourceStates"][0]["flows"][0]["amount"], 12.0,
        "independent resource state is retained"
    );

    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;
    assert_eq!(
        groups.base_of(guid(&twin["guid"])?),
        Some(guid(&base["guid"])?)
    );
    assert_eq!(groups.base_of(guid(&different["guid"])?), None);
    Ok(())
}
