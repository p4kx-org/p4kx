// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive whose shop components lie outside the shop folders, repeat neither the
//! component name nor the GUID of their Entity, or are named by a prefab.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestContainer, TestElement, TestEntry};

use crate::shops_fixture::{CLOTHING, EXCHANGE, LOOSE, REGISTRY, registry};
use crate::starmap_fixture;

pub(crate) const ITEM_A: &str = "d195eb47-24b8-45c0-a4d0-1771d60dc1a8";
pub(crate) const ITEM_B: &str = "70a98c94-9252-4fd3-a673-5a584339e273";

const CITY: &str = "Data\\ObjectContainers\\PU\\loc\\flagship\\city\\city_a.socpak";
const EXCHANGE_CONTAINER: &str = "Data\\ObjectContainers\\PU\\Shops\\commex\\tdd\\tdd_test.socpak";
const PREFAB: &str = "Data\\Prefabs\\shops\\food\\noodle_bar.xml";
const INVENTORIES: &str = "Data\\Scripts\\ShopInventories\\";
const CLASS: &str = "2845a83c-9c04-450f-99ce-43b8b7da6bec";
const CLOTHING_NAME: &str = "SCShop_Clothing_City";
const LOOSE_NAME: &str = "SCShop_Loose_City";
const FOOD_ID: &str = "{00004000-0000-4A00-F100-000000000080}";
const MIXED_ID: &str = "{00004000-0000-4A00-E100-000000000080}";

fn strings(parts: &[&str]) -> Vec<u8> {
    let mut bytes = b"CrChF\0".to_vec();
    for part in parts {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn metadata(stem: &str, guids: &[&str]) -> TestEntry {
    let root = guids
        .iter()
        .fold(TestElement::new("EntityComponentShop"), |root, guid| {
            root.child(TestElement::new("Entity").attribute("CryGUID", *guid))
        });
    TestEntry::stored(
        format!("{stem}\\metadata\\entitycomponentshop.xml"),
        root.to_xml(),
    )
}

fn editor(stem: &str, objects: &[(&str, &str)]) -> TestEntry {
    let root = objects
        .iter()
        .fold(TestElement::new("ObjectContainer"), |root, (name, guid)| {
            root.child(
                TestElement::new("Object")
                    .attribute("name", *name)
                    .attribute("type", "SCShop")
                    .attribute("guid", *guid),
            )
        });
    TestEntry::stored(format!("{stem}_editor.xml"), root.to_xml())
}

fn city() -> Vec<u8> {
    let soc = strings(&[
        CLOTHING_NAME,
        "1,2,3",
        "SCShop",
        CLASS,
        "EntityComponentShop",
        "SShopComponentParams",
        "offlineInventoryJSON",
        "Scripts/ShopInventories/Inv_Clothing_City.json",
        "ShopRackGUIDs",
        "ShopKioskGUIDs",
        LOOSE_NAME,
        "4,5,6",
        LOOSE,
        "Scripts/ShopInventories/Inv_Loose_City.json",
        "tail",
    ]);
    TestContainer::new()
        .entry(metadata("city_a", &[CLOTHING, LOOSE]))
        .entry(editor(
            "city_a",
            &[(CLOTHING_NAME, CLOTHING), (LOOSE_NAME, LOOSE)],
        ))
        .entry(TestEntry::stored("city_a.soc", soc))
        .build()
}

fn exchange() -> Vec<u8> {
    let soc = strings(&[
        EXCHANGE,
        "EntityComponentShop",
        "SShopComponentParams",
        "offlineInventoryJSON",
        "Scripts/ShopInventories/Temp_Outpost_ShopInventory.json",
        "ShopRackGUIDs",
    ]);
    TestContainer::new()
        .entry(metadata("tdd_test", &[EXCHANGE]))
        .entry(TestEntry::stored("tdd_test.soc", soc))
        .build()
}

fn prefab() -> Vec<u8> {
    let object = |id: &str, component: TestElement| {
        TestElement::new("Object")
            .attribute("Type", "Entity")
            .attribute("Id", id)
            .child(TestElement::new("PropertiesDataCore").child(component))
    };
    let food = TestElement::new("EntityComponentShop").attribute(
        "offlineInventoryXMLName",
        "Scripts/ShopInventories/Inv_Noodle.xml",
    );
    let mixed = TestElement::new("EntityComponentShop")
        .attribute(
            "offlineInventoryJSON",
            "Scripts/ShopInventories/Inv_Noodle.json",
        )
        .attribute(
            "offlineInventoryXMLName",
            "Scripts/ShopInventories/Inv_Loose_City.xml",
        );
    let objects = TestElement::new("Objects")
        .child(object(FOOD_ID, food))
        .child(object(MIXED_ID, mixed));
    TestElement::new("PrefabsLibrary")
        .child(TestElement::new("Prefab").child(objects))
        .to_xml()
        .into_bytes()
}

fn inventory(items: &[&str]) -> Vec<u8> {
    let elements: Vec<String> = items
        .iter()
        .map(|item| format!(r#"{{"ID":{{"ID":["{item}"]}},"BuyPrice":1.5}}"#))
        .collect();
    format!(r#"{{"Inventory":[{}]}}"#, elements.join(",")).into_bytes()
}

/// Builds the Archive with the registry, city container, exchange and prefab shop inventories.
pub(crate) fn builder() -> ArchiveBuilder {
    let mut builder = starmap_fixture::builder()
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(CITY, city()))
        .entry(TestEntry::stored(EXCHANGE_CONTAINER, exchange()))
        .entry(TestEntry::stored(PREFAB, prefab()));
    for (name, bytes) in [
        ("Inv_Clothing_City", inventory(&[ITEM_A])),
        ("Inv_Loose_City", inventory(&[ITEM_B, ITEM_A])),
        ("Inv_Noodle", inventory(&[ITEM_B])),
        ("Temp_Outpost_ShopInventory", inventory(&[ITEM_A])),
    ] {
        builder = builder.entry(TestEntry::stored(
            format!("{INVENTORIES}{name}.json"),
            bytes,
        ));
    }
    builder
}

/// Writes the Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    builder().write_to(&path)?;
    Ok(path)
}
