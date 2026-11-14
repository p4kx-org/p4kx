// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Item lists of shops in the Basetool catalog, through the public API.

use crate::shop_items_fixture::{self, ITEM_A, ITEM_B};
use crate::shops_fixture::{CLOTHING, EXCHANGE, FOOD, LOOSE, MIXED};

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, CraftingWarning, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn build_with(
    write: impl FnOnce(&Path) -> Result<PathBuf, Box<dyn Error>>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(write(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn build() -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_with(shop_items_fixture::write_archive)
}

fn shops_of<'a>(catalog: &'a Value, entity: &str) -> Vec<&'a Value> {
    catalog["shops"].as_array().map_or_default(|shops| {
        shops
            .iter()
            .filter(|shop| shop["entityGuid"] == entity)
            .collect()
    })
}

#[test]
fn a_named_inventory_gives_ordered_item_guids_without_prices() -> TestResult {
    let (_, catalog) = build()?;
    let shops = shops_of(&catalog, EXCHANGE);
    assert_eq!(shops.len(), 1);
    assert_eq!(shops[0]["inventory"], "inv_commex");
    assert_eq!(shops[0]["itemGuids"], serde_json::json!([ITEM_B, ITEM_A]));
    let text = serde_json::to_string(&catalog["shops"])?;
    for forbidden in ["BuyPrice", "SellPrice", "CurrentInventory", "Rental"] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    Ok(())
}

#[test]
fn an_xml_name_resolves_to_the_json_file_and_the_flat_shape_is_read() -> TestResult {
    let (_, catalog) = build()?;
    let shops = shops_of(&catalog, LOOSE);
    assert_eq!(shops.len(), 1);
    assert_eq!(shops[0]["inventory"], "inv_loose");
    assert_eq!(shops[0]["itemGuids"], serde_json::json!([ITEM_A]));
    Ok(())
}

#[test]
fn a_shop_without_a_usable_inventory_has_no_list() -> TestResult {
    let (_, catalog) = build()?;
    for entity in [CLOTHING, FOOD, MIXED] {
        for shop in shops_of(&catalog, entity) {
            assert!(shop["inventory"].is_null(), "{entity}");
            assert!(shop["itemGuids"].is_null(), "{entity}");
        }
    }
    Ok(())
}

#[test]
fn missing_unreadable_and_ambiguous_inventories_give_no_list_and_are_counted() -> TestResult {
    let (built, _) = build()?;
    let shops = built.summary().shops();
    assert_eq!(shops.with_items, 2);
    assert_eq!(shops.inventory_missing, 2, "the clothing shop twice");
    assert_eq!(shops.inventories_missing, 1);
    assert_eq!(shops.inventory_unreadable, 1);
    assert_eq!(shops.inventories_unreadable, 1);
    assert_eq!(shops.inventory_ambiguous, 1);
    assert_eq!(shops.inventory_unscanned, 0);
    assert_eq!(shops.soc_components, 6);
    let warnings = built.summary().warnings();
    let details = warnings
        .iter()
        .find_map(|warning| match warning {
            CraftingWarning::UnreadableShopInventories { count: 1, details } => Some(details),
            _ => None,
        })
        .ok_or_else(|| format!("{warnings:?}"))?;
    assert!(
        matches!(details.as_slice(), [detail]
            if detail.to_ascii_lowercase().contains("inv_noodle.json")
                && detail.ends_with("the file is no inventory JSON")),
        "the warning names the file and why: {details:?}"
    );
    assert!(
        warnings.contains(&CraftingWarning::MissingShopInventories(1)),
        "{warnings:?}"
    );
    assert!(!warnings.contains(&CraftingWarning::NoShopComponents));
    Ok(())
}

#[test]
fn a_soc_without_a_shop_component_is_unscanned_and_warns() -> TestResult {
    let (built, catalog) = build_with(shop_items_fixture::write_archive_without_components)?;
    let shops = built.summary().shops();
    assert_eq!(shops.containers_with_shop, 1);
    assert_eq!(shops.soc_components, 0);
    assert_eq!(shops.inventory_unscanned, 1);
    assert_eq!(shops.with_items, 0);
    assert!(
        built
            .summary()
            .warnings()
            .contains(&CraftingWarning::NoShopComponents),
        "{:?}",
        built.summary().warnings()
    );
    assert!(
        shops_of(&catalog, EXCHANGE)
            .iter()
            .all(|shop| shop["itemGuids"].is_null() && shop["kind"] == "commex"),
        "the kind does not depend on the .soc"
    );
    Ok(())
}
