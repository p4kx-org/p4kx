// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups read without a catalog build agree with the `variantOf` of the catalog.

use crate::{item_stats_fixture, variants_fixture};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, Guid, Kind, build_catalog, read_variant_groups};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::catalog_variants::decision_reader::{VariantTarget, variant_target};

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
        CatalogOptions {
            publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        }
        .with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;

    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;

    let mut checked = 0;
    for kind in [Kind::Ships, Kind::Items] {
        for entity in catalog_v3_consumer::rows(&catalog, kind)? {
            if kind == Kind::Ships
                && entity.pointer("/variantOf/state").and_then(Value::as_str) == Some("unreadable")
            {
                assert_eq!(
                    entity
                        .pointer("/variantOf/sourceSetComplete")
                        .and_then(Value::as_bool),
                    Some(false),
                    "the legacy finder omits explicitly undecidable Ship comparisons"
                );
                continue;
            }
            let base = variant_target(entity, kind)?
                .map(|target| target.guid.parse())
                .transpose()?;
            let own = guid(catalog_v3_consumer::field(entity, kind, "guid")?)?;
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
    let built = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let items = catalog["items"].as_array().ok_or("catalog items")?;
    let item = |name: &str| {
        items
            .iter()
            .find(|item| item.pointer("/className/value").and_then(Value::as_str) == Some(name))
            .ok_or_else(|| format!("missing {name}"))
    };
    let base = item("shield_base")?;
    let twin = item("shield_base_twin")?;
    let different = item("shield_different")?;
    for row in [base, twin, different] {
        assert!(
            row.get("stats").is_none(),
            "the Item wire excludes private comparison inputs"
        );
    }
    assert!(
        variant_target(base, Kind::Items)?.is_none(),
        "the base retains its identity"
    );
    let VariantTarget {
        guid: target_guid,
        record_index,
    } = variant_target(twin, Kind::Items)?.ok_or("equal shield target")?;
    assert_eq!(
        Some(target_guid),
        base.pointer("/guid/value").and_then(Value::as_str),
        "the equal shield is grouped"
    );
    assert_eq!(
        Some(u64::from(record_index)),
        base.pointer("/recordIndex/value").and_then(Value::as_u64),
        "the shield points at the exact physical base"
    );
    assert!(
        variant_target(different, Kind::Items)?.is_none(),
        "a shield with different authored health stays separate"
    );
    check_resource_configuration(&catalog, item("shield_with_resources")?)?;

    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;
    assert_eq!(
        groups.base_of(guid(&twin["guid"]["value"])?),
        Some(guid(&base["guid"]["value"])?)
    );
    assert_eq!(groups.base_of(guid(&different["guid"]["value"])?), None);
    Ok(())
}

fn check_resource_configuration(catalog: &Value, resource_item: &Value) -> TestResult {
    assert!(
        resource_item.get("stats").is_none(),
        "the shield and resource component publish their own independent configuration rows"
    );
    let owner_guid = resource_item
        .pointer("/guid/value")
        .ok_or("the shield's original GUID")?;
    let resource = catalog
        .pointer("/configurations/resourceComponents/rows")
        .and_then(Value::as_array)
        .ok_or("canonical resource components")?
        .iter()
        .find(|row| row.pointer("/ownerGuid/value") == Some(owner_guid))
        .ok_or("the shield's original resource component")?;
    assert_eq!(
        resource.pointer("/states/value/0/value/deltas/value/0/value/generation/value/resourceAmountPerSecond/value/units/value/finite"),
        Some(&serde_json::json!(12.0)),
        "the independent original resource amount is retained"
    );
    Ok(())
}
