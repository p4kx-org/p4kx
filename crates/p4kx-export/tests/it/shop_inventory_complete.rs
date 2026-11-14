// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufReader, Read};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Entry};
use p4kx_testkit::{TestElement, TestEntry};
use serde_json::{Value, json};

use super::{TestResult, shop};
use crate::shop_items_rest_fixture::ITEM_B;
use crate::shops_fixture::{FOOD, REGISTRY, registry};
use crate::starmap_fixture;

const BOUNDARY: usize = 16 << 20;
const PATH_BOUNDARY: usize = 256;
const PREFAB: &str = "Data\\Prefabs\\shops\\inventory_complete.xml";
const REFERENCE: &str = "Scripts/ShopInventories/Inv_FullCatalog.json";
const INVENTORY: &str = "Data\\Scripts\\ShopInventories\\Inv_FullCatalog.json";

fn prefab(reference: &str) -> String {
    let component =
        TestElement::new("EntityComponentShop").attribute("offlineInventoryJSON", reference);
    let entity = TestElement::new("Object")
        .attribute("Type", "Entity")
        .attribute("Id", "{00004000-0000-4A00-F100-000000000080}")
        .child(TestElement::new("PropertiesDataCore").child(component));
    TestElement::new("PrefabsLibrary")
        .child(TestElement::new("Prefab").child(TestElement::new("Objects").child(entity)))
        .to_xml()
}

fn original<'archive>(archive: &'archive Archive, path: &str) -> Option<&'archive Entry> {
    archive.entries().iter().find(|entry| entry.path() == path)
}

fn assert_original_inventory(archive: &Archive, reference: &str, size: usize) -> TestResult {
    let prefab = original(archive, PREFAB).ok_or("original prefab")?;
    let mut xml = String::new();
    archive
        .open_entry_verified(prefab)?
        .read_to_string(&mut xml)?;
    let document = roxmltree::Document::parse(&xml)?;
    let component = document
        .descendants()
        .find(|node| node.has_tag_name("EntityComponentShop"))
        .ok_or("original shop component")?;
    assert_eq!(
        component.attribute("offlineInventoryJSON"),
        Some(reference),
        "the original prefab holds the complete authored path"
    );

    let entry = original(archive, INVENTORY).ok_or("original inventory")?;
    assert_eq!(
        entry.uncompressed_size(),
        u64::try_from(size)?,
        "the Archive declares the complete decoded inventory size"
    );
    let bytes = archive.open_entry_verified(entry)?.reject_overlong();
    let inventory: Value = serde_json::from_reader(BufReader::new(bytes))?;
    assert_eq!(
        inventory.pointer("/Inventory/0/ID/ID/0"),
        Some(&json!(ITEM_B)),
        "the original complete inventory contains its authored item"
    );
    Ok(())
}

fn assert_inventory_survives(size: usize, reference: &str) -> TestResult {
    let json = format!(r#"{{"Inventory":[{{"ID":{{"ID":["{ITEM_B}"]}}}}]}}"#);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size)?;
    bytes.resize(size.checked_sub(json.len()).ok_or("inventory size")?, b' ');
    bytes.extend_from_slice(json.as_bytes());
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    starmap_fixture::builder()
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(PREFAB, prefab(reference)))
        .entry(TestEntry::deflated(INVENTORY, bytes))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    assert_original_inventory(&archive, reference, size)?;

    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let food = shop(&catalog, FOOD)?;
    assert_eq!(
        food["inventory"], "inv_fullcatalog",
        "actual prefab selection"
    );
    assert_eq!(food["itemGuids"], json!([ITEM_B]), "{size}-byte inventory");
    Ok(())
}

#[test]
fn reads_a_selected_inventory_at_the_old_boundary() -> TestResult {
    assert_inventory_survives(BOUNDARY, REFERENCE)
}

#[test]
fn reads_a_selected_inventory_beyond_the_old_boundary() -> TestResult {
    assert_inventory_survives(BOUNDARY + 1, REFERENCE)
}

fn assert_prefab_inventory_path_survives(path_size: usize) -> TestResult {
    let prefix = "Scripts/ShopInventories/";
    let leaf = "/Inv_FullCatalog.json";
    let directory_size = path_size
        .checked_sub(prefix.len() + leaf.len())
        .ok_or("authored inventory path size")?;
    let reference = format!("{prefix}{}{leaf}", "d".repeat(directory_size));
    assert_eq!(reference.len(), path_size, "exact original path size");
    assert_inventory_survives(128, &reference)
}

#[test]
fn reads_a_prefab_inventory_path_at_the_old_boundary() -> TestResult {
    assert_prefab_inventory_path_survives(PATH_BOUNDARY)
}

#[test]
fn reads_a_prefab_inventory_path_beyond_the_old_boundary() -> TestResult {
    assert_prefab_inventory_path_survives(PATH_BOUNDARY + 1)
}
