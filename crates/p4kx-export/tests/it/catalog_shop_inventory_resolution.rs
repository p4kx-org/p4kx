// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read as _;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::Archive;
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as health;
use crate::shop_items_rest_fixture::ITEM_A;
use crate::shops_fixture::REGISTRY;
use crate::starmap_fixture;

#[path = "shop_inventory_ambiguous_targets.rs"]
mod ambiguous_targets;
#[path = "shop_inventory_duplicate_physical.rs"]
mod duplicate_physical;
#[path = "shop_inventory_rejected_path.rs"]
mod rejected_path;
#[path = "shop_inventory_resolution_sources.rs"]
mod source_checks;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const REGISTRY_ROOT: &str = "4a269932-183e-362d-aacc-188e7debbea9";
const PREFAB: &str = "Data\\Prefabs\\shops\\inventory_resolution.xml";
const CONTAINER: &str = "Data\\ObjectContainers\\PU\\Shops\\inventory_resolution\\terminal.socpak";
const METADATA: &str = "terminal\\metadata\\entitycomponentshop.xml";
const SOC: &str = "terminal.soc";
const INVENTORIES: &str = "Data\\Scripts\\ShopInventories\\";
const RESOLVED_PATHS: [&str; 2] = [
    "Data/Scripts/ShopInventories/Inv_Resolved.json",
    "Scripts\\ShopInventories\\Inv_Resolved.xml",
];
const AMBIGUOUS_PATHS: [&str; 3] = [
    "Scripts/ShopInventories/Inv_Left.json",
    "Data\\Scripts\\ShopInventories\\INV_LEFT.xml",
    "Scripts/ShopInventories/Inv_Right.xml",
];

struct ShopCase {
    status: &'static str,
    paths: &'static [&'static str],
}

const CASES: [ShopCase; 7] = [
    ShopCase {
        status: "resolved",
        paths: &RESOLVED_PATHS,
    },
    ShopCase {
        status: "placeholder",
        paths: &["Scripts/ShopInventories/Temp_Outpost_ShopInventory.json"],
    },
    ShopCase {
        status: "ambiguous",
        paths: &AMBIGUOUS_PATHS,
    },
    ShopCase {
        status: "missing",
        paths: &["Scripts/ShopInventories/Inv_Missing.json"],
    },
    ShopCase {
        status: "unreadable",
        paths: &["Scripts/ShopInventories/Inv_Unreadable.json"],
    },
    ShopCase {
        status: "unscanned",
        paths: &[],
    },
    ShopCase {
        status: "noDeclaredFile",
        paths: &[],
    },
];

fn entity(index: usize) -> String {
    format!("4a000000-0000-4000-8000-{index:012x}")
}

fn key(index: usize) -> String {
    format!("{REGISTRY_ROOT},{}", entity(index))
}

fn registry() -> String {
    let mut rows = TestElement::new("ShopBackups");
    for (index, case) in CASES.iter().enumerate() {
        rows = rows.child(
            TestElement::new("ShopBackup")
                .attribute("ID", key(index))
                .attribute("SuperName", format!("Root,SCShop_{}", case.status))
                .attribute("ShopName", case.status),
        );
    }
    TestElement::new("BackupData")
        .child(
            TestElement::new("LayerBackups").child(
                TestElement::new("LayerBackup")
                    .attribute("PlatformName", "StantonSystem")
                    .child(rows),
            ),
        )
        .to_xml()
}

fn prefab(rejected: Option<&str>) -> String {
    let mut objects = TestElement::new("Objects");
    for (index, case) in CASES
        .iter()
        .enumerate()
        .filter(|(_, case)| !case.paths.is_empty())
    {
        let mut component = TestElement::new("EntityComponentShop");
        for (ordinal, path) in case.paths.iter().enumerate() {
            component = component.attribute(format!("offlineInventory{ordinal}"), *path);
        }
        if index == 0
            && let Some(path) = rejected
        {
            component = component.attribute("offlineInventoryRejected", path);
        }
        let id = format!("{{00004000-0000-4A00-{index:02X}00-000000000080}}");
        objects = objects.child(
            TestElement::new("Object")
                .attribute("Type", "Entity")
                .attribute("Id", id)
                .child(TestElement::new("PropertiesDataCore").child(component)),
        );
    }
    TestElement::new("PrefabsLibrary")
        .child(TestElement::new("Prefab").child(objects))
        .to_xml()
}

fn container() -> Vec<u8> {
    let metadata = TestElement::new("EntityComponentShop")
        .child(TestElement::new("Entity").attribute("CryGUID", entity(5)))
        .child(TestElement::new("Entity").attribute("CryGUID", entity(6)))
        .to_xml();
    let soc = format!("CrChF\0{}\0EntityComponentShop\0", entity(6));
    TestContainer::new()
        .entry(TestEntry::stored(METADATA, metadata))
        .entry(TestEntry::stored(SOC, soc))
        .build()
}

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let mut builder = starmap_fixture::builder()
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(PREFAB, prefab(None)))
        .entry(TestEntry::stored(CONTAINER, container()));
    for name in [
        "Inv_Resolved",
        "Inv_Left",
        "Inv_Right",
        "Temp_Outpost_ShopInventory",
    ] {
        let source = format!(r#"{{"Inventory":[{{"ID":{{"ID":["{ITEM_A}"]}}}}]}}"#);
        builder = builder.entry(TestEntry::stored(
            format!("{INVENTORIES}{name}.json"),
            source,
        ));
    }
    builder
        .entry(
            TestEntry::stored(format!("{INVENTORIES}Inv_Unreadable.json"), "{invalid").encrypted(),
        )
        .write_to(&path)?;
    Ok((
        dir,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn original(archive: &Archive, path: &str) -> TestResult<String> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("actual original Entry")?;
    let mut text = String::new();
    archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_string(&mut text)?;
    Ok(text)
}

#[test]
fn shop_inventory_resolution_exposes_each_original_status() -> TestResult {
    let (_dir, archive) = archive()?;
    source_checks::assert_original_sources(&archive)?;
    let alternatives = ambiguous_targets::assert_original(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let summary = built.summary().shops();
    assert_eq!(
        summary.shops,
        CASES.len(),
        "every actual registry row is emitted"
    );
    assert_eq!(summary.with_items, 1, "one selected readable legacy list");
    assert_eq!(
        summary.inventory_placeholder, 1,
        "one placeholder selection"
    );
    assert_eq!(summary.inventory_ambiguous, 1, "one actual disagreement");
    assert_eq!(summary.inventory_missing, 1, "one absent physical target");
    assert_eq!(
        summary.inventory_unreadable, 1,
        "one invalid physical target"
    );
    assert_eq!(
        summary.inventory_unscanned, 1,
        "one registered Entity without readable component"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = health::rows(&catalog, "/declarations/shopInventoryResolution/rows")?;
    assert_eq!(rows.len(), CASES.len(), "one exact resolution per shop");
    for (index, case) in CASES.iter().enumerate() {
        let key = key(index);
        let row = resolution_row(rows, &key)?;
        assert_state(&catalog, row, case, &key)?;
    }
    assert_selected_targets(&catalog, rows)?;
    ambiguous_targets::assert_export(&catalog, rows, alternatives)
}

fn resolution_row<'catalog>(rows: &'catalog [Value], key: &str) -> TestResult<&'catalog Value> {
    rows.iter()
        .find(|row| row.pointer("/shopKey/value").and_then(Value::as_str) == Some(key))
        .ok_or_else(|| format!("missing resolution for {key}").into())
}

fn assert_state(catalog: &Value, row: &Value, case: &ShopCase, key: &str) -> TestResult {
    let shop_key = health::present(row, "shopKey", &json!(key), Some(false))?;
    assert!(
        uses(catalog, shop_key)?
            .iter()
            .any(|used| used.pointer("/location/field").and_then(Value::as_str) == Some("@ID")),
        "the original registry ID attribute supplies the shop key"
    );
    let status = health::cell(row, "status", "present")?;
    assert_eq!(
        status["value"], case.status,
        "the actual resolution for {key}"
    );
    let candidates = health::rows(row, "/candidates/value")?;
    if case.paths.is_empty() {
        assert_marker(catalog, candidates, case.status)?;
    } else {
        assert_paths(catalog, candidates, case.paths)?;
    }
    if case.status == "unreadable" {
        assert_eq!(
            status["sourceSetComplete"], false,
            "failed payload is not a complete origin"
        );
        assert!(
            status["containsEncryptedSource"].is_null(),
            "failed encryption is not a payload contribution"
        );
    }
    Ok(())
}

fn assert_paths(catalog: &Value, candidates: &[Value], expected: &[&str]) -> TestResult {
    assert_eq!(
        candidates.len(),
        expected.len(),
        "all authored candidate occurrences"
    );
    for (candidate, path) in candidates.iter().zip(expected) {
        let value = health::at(candidate, "/value")?;
        assert_eq!(
            value["type"], "file",
            "a candidate is an actual declared file"
        );
        let path = health::present(value, "sourcePath", &json!(path), Some(false))?;
        assert!(
            uses(catalog, path)?.iter().any(|used| {
                used.pointer("/location/namespace").and_then(Value::as_str) == Some("xml")
                    && used
                        .pointer("/location/field")
                        .and_then(Value::as_str)
                        .is_some_and(|field| field.starts_with("@offlineInventory"))
            }),
            "the exact authored path comes from a prefab attribute"
        );
    }
    Ok(())
}

fn assert_marker(catalog: &Value, candidates: &[Value], status: &str) -> TestResult {
    let [candidate] = candidates else {
        return Err("one exact observed component or metadata selector".into());
    };
    assert_eq!(
        health::at(candidate, "/value/type")?,
        &json!(status),
        "the original negative source kind"
    );
    let marker_offset = ("CrChF".len() + entity(6).len() + 2).to_string();
    let source = uses(catalog, candidate)?;
    let matched = match status {
        "noDeclaredFile" => source.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("shopSoc")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("string")
                && used.pointer("/location/indices") == Some(&json!([marker_offset]))
        }),
        "unscanned" => source.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("xml")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("@CryGUID")
        }),
        _ => return Err("unexpected negative inventory state".into()),
    };
    assert!(matched, "the actual marker or metadata location survives");
    Ok(())
}

fn uses<'catalog>(catalog: &'catalog Value, cell: &Value) -> TestResult<Vec<&'catalog Value>> {
    let refs = health::rows(cell, "/sourceUsesRefs")?;
    Ok(health::rows(catalog, "/provenance/uses")?
        .iter()
        .filter(|used| used.get("id").is_some_and(|id| refs.contains(id)))
        .collect())
}

fn assert_selected_targets(catalog: &Value, rows: &[Value]) -> TestResult {
    let physical = health::rows(catalog, "/definitions/shopInventories/rows")?;
    for (index, expected) in [
        (0, "Inv_Resolved.json"),
        (1, "Temp_Outpost_ShopInventory.json"),
        (4, "Inv_Unreadable.json"),
    ] {
        let row = resolution_row(rows, &key(index))?;
        let targets = health::rows(row, "/physicalTargets/value")?;
        let [target] = targets else {
            return Err("one actual physical inventory".into());
        };
        let path = format!("{INVENTORIES}{expected}");
        let file = physical
            .iter()
            .find(|entry| entry.pointer("/sourcePath/value") == Some(&json!(path)))
            .ok_or("selected original inventory definition")?;
        assert_eq!(
            health::at(target, "/value/inputRef")?,
            health::at(file, "/inputRef")?,
            "same actual physical owner"
        );
        assert_eq!(
            health::at(target, "/value/entryIndex")?,
            health::at(file, "/entryIndex")?,
            "same directory occurrence"
        );
        assert_eq!(
            target["containsEncryptedSource"], false,
            "directory identity has its own source"
        );
    }
    Ok(())
}
