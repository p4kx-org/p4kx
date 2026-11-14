// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shops in the Basetool catalog, through the public API.

use crate::shops_fixture::{self, EXCHANGE, LOOSE, write_archive};
use crate::starmap_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, CraftingWarning, Filter, Kind,
    build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);
const PLANET: &str = "10000000-0000-4000-8000-000000000002";
const LOCATIONS: [&str; 6] = [
    "41000000-0000-4000-8000-000000000001",
    "41000000-0000-4000-8000-000000000002",
    "41000000-0000-4000-8000-000000000003",
    "41000000-0000-4000-8000-000000000004",
    "41000000-0000-4000-8000-000000000005",
    "41000000-0000-4000-8000-000000000006",
];
const ROOT: &str = "4a269932-183e-362d-aacc-188e7debbea9,4ce1f681-b479-5269-0536-3d7f02fdccaf";

fn build(
    archive: impl FnOnce(&std::path::Path) -> Result<std::path::PathBuf, Box<dyn Error>>,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn shop<'a>(catalog: &'a Value, location_id: &str) -> Result<&'a Value, Box<dyn Error>> {
    catalog["shops"]
        .as_array()
        .ok_or("shops")?
        .iter()
        .find(|shop| {
            shop["key"]
                .as_str()
                .is_some_and(|key| key.contains(location_id))
        })
        .ok_or_else(|| format!("the shop at {location_id}").into())
}

#[test]
fn a_shop_is_a_registry_entry_with_its_chain_as_the_key() -> TestResult {
    let (_, catalog) = build(write_archive, None)?;
    let keys: Vec<&str> = catalog["shops"]
        .as_array()
        .ok_or("shops")?
        .iter()
        .filter_map(|shop| shop["key"].as_str())
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "ordered by key");
    assert_eq!(
        keys.len(),
        6,
        "the malformed and the repeated entry are left out"
    );
    let first = shop(&catalog, LOCATIONS[0])?;
    assert_eq!(
        first["key"],
        format!("{ROOT},{},{EXCHANGE}", LOCATIONS[0]).as_str()
    );
    assert_eq!(first["entityGuid"], EXCHANGE);
    assert_eq!(first["system"], "stanton");
    assert_eq!(first["entityName"], "SCShop_CommEx_TDD_Hurston");
    assert_eq!(first["registryName"], "L_TDD_Hurston");
    assert_eq!(
        first["defaultValue"],
        "d0000000-0000-4000-8000-000000000001"
    );
    assert_eq!(
        first["containers"],
        serde_json::json!(["OOC_Stanton", "OOC_Stanton_1_Hurston", "Hurston"])
    );
    assert_eq!(first["body"], "OOC_Stanton_1_Hurston");
    assert_eq!(first["location"], "Hurston");
    Ok(())
}

#[test]
fn the_location_name_links_to_a_starmap_object_by_entity_or_class_name() -> TestResult {
    let (_, catalog) = build(write_archive, None)?;
    for (location, expected) in [
        (LOCATIONS[0], Some(PLANET)),
        (LOCATIONS[1], Some(PLANET)),
        (LOCATIONS[2], Some(PLANET)),
        (LOCATIONS[3], None),
        (LOCATIONS[5], None),
    ] {
        let linked = &shop(&catalog, location)?["starmapObjectGuid"];
        match expected {
            Some(guid) => assert_eq!(linked, guid, "{location}"),
            None => assert!(linked.is_null(), "{location}"),
        }
    }
    Ok(())
}

#[test]
fn kind_and_brand_come_from_the_container_that_names_the_entity() -> TestResult {
    let (_, catalog) = build(write_archive, None)?;
    let expected = [
        (LOCATIONS[0], Some("commex"), Some("tdd")),
        (LOCATIONS[1], Some("clothing"), Some("mcraes")),
        (LOCATIONS[2], Some("clothing"), Some("mcraes")),
        (LOCATIONS[3], Some("shopfood"), None),
        (LOCATIONS[4], None, None),
        (LOCATIONS[5], None, None),
    ];
    for (location, kind, brand) in expected {
        let shop = shop(&catalog, location)?;
        assert_eq!(
            shop["kind"],
            kind.map_or(Value::Null, Value::from),
            "{location}"
        );
        assert_eq!(
            shop["brand"],
            brand.map_or(Value::Null, Value::from),
            "{location}"
        );
    }
    assert_ne!(
        shop(&catalog, LOCATIONS[1])?["key"],
        shop(&catalog, LOCATIONS[2])?["key"],
        "one module placed twice is two shops"
    );
    Ok(())
}

#[test]
fn commodity_exchanges_are_marked_and_empty_names_are_null() -> TestResult {
    let (_, catalog) = build(write_archive, None)?;
    for (location, marked) in [
        (LOCATIONS[0], true),
        (LOCATIONS[1], false),
        (LOCATIONS[3], true),
        (LOCATIONS[5], false),
    ] {
        assert_eq!(
            shop(&catalog, location)?["commodityExchange"],
            marked,
            "{location}"
        );
    }
    let pyro = shop(&catalog, LOCATIONS[5])?;
    assert_eq!(pyro["system"], "pyro");
    assert_eq!(pyro["entityGuid"], LOOSE);
    assert!(pyro["registryName"].is_null());
    assert!(pyro["defaultValue"].is_null());
    assert_eq!(pyro["body"], "OOC_Stanton_1_Hurston");
    Ok(())
}

#[test]
fn the_summary_counts_what_was_read_and_linked() -> TestResult {
    let (built, _) = build(write_archive, None)?;
    let shops = built.summary().shops();
    assert_eq!(shops.shops, 6);
    assert_eq!(shops.registry_entries, 8);
    assert_eq!((shops.malformed, shops.duplicate), (1, 1));
    assert_eq!((shops.linked, shops.unlinked), (4, 2));
    assert_eq!(
        shops.unlinked_locations,
        ["Nowhere_Place", "rs_ext_pyro3_leo"]
    );
    assert_eq!(shops.containers_listed, 7);
    assert_eq!(shops.containers_unreadable, 1);
    assert_eq!(shops.containers_with_shop, 5);
    assert_eq!(
        (shops.with_kind, shops.mixed_kind, shops.without_container),
        (4, 1, 1)
    );
    assert_eq!(shops.commodity_exchanges, 2);
    assert_eq!(
        shops.systems,
        [("pyro".to_owned(), 1), ("stanton".to_owned(), 5)]
    );
    assert_eq!(
        shops.kinds,
        [
            ("clothing".to_owned(), 2),
            ("commex".to_owned(), 1),
            ("shopfood".to_owned(), 1)
        ]
    );
    let kind = built.summary().kind(Kind::Shops).ok_or("kind")?;
    assert_eq!(kind.count, 6);
    Ok(())
}

#[test]
fn an_archive_without_a_registry_but_with_a_starmap_warns() -> TestResult {
    let (built, catalog) = build(starmap_fixture::write_archive, None)?;
    assert_eq!(catalog["shops"], serde_json::json!([]));
    assert!(!built.summary().shops().registry_unreadable);
    assert!(built.summary().shops().registry_missing);
    assert_eq!(built.summary().shops().registry_entries, 0);
    assert!(
        built
            .summary()
            .warnings()
            .contains(&CraftingWarning::MissingShopRegistry),
        "{:?}",
        built.summary().warnings()
    );
    assert!(!built.summary().is_complete());
    Ok(())
}

#[test]
fn an_unreadable_registry_warns_instead_of_looking_like_no_shops() -> TestResult {
    for bytes in [
        b"not xml at all".to_vec(),
        b"<BackupData><LayerBackups".to_vec(),
        vec![0xff, 0xfe, 0xfd],
    ] {
        let (built, catalog) = build(
            |dir| shops_fixture::write_archive_with_registry(dir, &bytes),
            None,
        )?;
        assert_eq!(catalog["shops"], serde_json::json!([]));
        assert!(built.summary().shops().registry_unreadable, "{bytes:?}");
        assert!(!built.summary().shops().registry_missing);
        assert!(
            built
                .summary()
                .warnings()
                .contains(&CraftingWarning::UnreadableShopRegistry),
            "{:?}",
            built.summary().warnings()
        );
        assert!(!built.summary().is_complete());
    }
    Ok(())
}

#[test]
fn unreadable_shop_containers_warn_with_their_count() -> TestResult {
    let (built, _) = build(write_archive, None)?;
    assert!(
        built
            .summary()
            .warnings()
            .contains(&CraftingWarning::UnreadableShopContainers(1)),
        "{:?}",
        built.summary().warnings()
    );
    assert!(
        !built
            .summary()
            .warnings()
            .contains(&CraftingWarning::UnreadableShopRegistry)
    );
    Ok(())
}

#[test]
fn a_filter_leaves_the_shops_out_and_makes_the_kind_partial() -> TestResult {
    let filter = Filter::new("*/starmap/*");
    let (_, catalog) = build(write_archive, Some(&filter))?;
    assert_eq!(catalog["shops"], serde_json::json!([]));
    assert_eq!(catalog["completeness"]["shops"]["status"], "partial");
    Ok(())
}
