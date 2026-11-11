// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Item lists of shops whose component lies outside the shop folders, in a later position of a
//! container file or in a prefab, through the public API.

use crate::shop_items_rest_fixture::{self, ITEM_A, ITEM_B};
use crate::shops_fixture::{CLOTHING, EXCHANGE, FOOD, LOOSE, MIXED};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

#[path = "shop_inventory_complete.rs"]
mod inventory_complete;
#[path = "shop_inventory_normalized.rs"]
mod inventory_normalized;

type TestResult = Result<(), Box<dyn Error>>;

fn shop<'a>(catalog: &'a Value, entity: &str) -> Result<&'a Value, Box<dyn Error>> {
    catalog["shops"]
        .as_array()
        .and_then(|shops| shops.iter().find(|shop| shop["entityGuid"] == entity))
        .ok_or_else(|| format!("no shop {entity}").into())
}

#[test]
fn components_in_any_container_and_prefabs_give_lists_and_placeholders_none() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = shop_items_rest_fixture::write_archive(dir.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;

    let clothing = shop(&catalog, CLOTHING)?;
    assert_eq!(clothing["inventory"], "inv_clothing_city", "tied by name");
    assert_eq!(clothing["itemGuids"], serde_json::json!([ITEM_A]));
    assert!(clothing["kind"].is_null(), "the folder gives no kind");

    let loose = shop(&catalog, LOOSE)?;
    assert_eq!(loose["inventory"], "inv_loose_city", "no property name");
    assert_eq!(loose["itemGuids"], serde_json::json!([ITEM_B, ITEM_A]));

    let food = shop(&catalog, FOOD)?;
    assert_eq!(food["inventory"], "inv_noodle", "named by the prefab");
    assert_eq!(food["itemGuids"], serde_json::json!([ITEM_B]));

    let exchange = shop(&catalog, EXCHANGE)?;
    assert!(exchange["inventory"].is_null());
    assert!(exchange["itemGuids"].is_null());
    assert_eq!(exchange["kind"], "commex", "kind does not depend on it");

    let mixed = shop(&catalog, MIXED)?;
    assert!(
        mixed["inventory"].is_null(),
        "a prefab that names two files"
    );
    assert!(mixed["itemGuids"].is_null());

    let summary = built.summary().shops();
    assert_eq!(summary.with_items, 4, "{summary:?}");
    assert_eq!(summary.inventory_placeholder, 1, "{summary:?}");
    assert_eq!(summary.other_containers_with_shop, 1, "{summary:?}");
    assert_eq!(summary.prefab_entities, 2, "{summary:?}");
    assert_eq!(summary.prefab_ambiguous, 1, "{summary:?}");
    assert_eq!(summary.prefab_only_shops, 1, "{summary:?}");
    assert_eq!(summary.inventory_unscanned, 0, "{summary:?}");
    let text = serde_json::to_string(&catalog["shops"])?;
    assert!(!text.contains("BuyPrice"));
    Ok(())
}
