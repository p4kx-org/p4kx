// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{Archive, SourceEncryption};
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{TestResult, original, resolution_row};
use crate::health_origin_assertions as health;
use crate::shop_items_rest_fixture::{ITEM_A, ITEM_B};
use crate::shops_fixture::{self, EXCHANGE, REGISTRY};
use crate::starmap_fixture;

const CONTAINER: &str = "Data\\ObjectContainers\\PU\\Shops\\commex\\tdd\\tdd_test.socpak";
const SOC: &str = "tdd_test.soc";
const PROPERTY: &str = "offlineInventoryJSON";
const DECLARED_PATH: &str = "Scripts/ShopInventories/Inv_CommEx.xml";
const FILES: [(&str, &str); 2] = [
    ("Data\\Scripts\\ShopInventories\\Inv_CommEx.json", ITEM_A),
    ("data\\scripts\\shopinventories\\inv_commex.JSON", ITEM_B),
];

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let metadata = TestElement::new("EntityComponentShop")
        .child(TestElement::new("Entity").attribute("CryGUID", EXCHANGE))
        .to_xml();
    let soc = format!(
        "CrChF\0{EXCHANGE}\0EntityComponentShop\0SShopComponentParams\0{PROPERTY}\0{DECLARED_PATH}\0ShopRackGUIDs\0"
    );
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "tdd_test\\metadata\\entitycomponentshop.xml",
            metadata,
        ))
        .entry(TestEntry::stored(SOC, soc))
        .build();
    let mut builder = starmap_fixture::builder()
        .entry(TestEntry::stored(REGISTRY, shops_fixture::registry()))
        .entry(TestEntry::stored(CONTAINER, container));
    for (name, item) in FILES {
        let payload = json!({"Inventory": [{"ID": {"ID": [item]}}]});
        builder = builder.entry(TestEntry::stored(name, serde_json::to_vec(&payload)?));
    }
    builder.write_to(&path)?;
    Ok((
        dir,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn assert_original(archive: &Archive) -> TestResult<[(usize, &str); 2]> {
    let socpak = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == CONTAINER)
        .ok_or("actual shop container")?;
    let container = archive.open_container(socpak)?;
    let entry = container
        .entries()
        .iter()
        .find(|entry| entry.path() == SOC)
        .ok_or("actual nested SOC entry")?;
    let source = container
        .observed_entry(entry)
        .ok_or("observed nested SOC entry")?;
    assert_eq!(
        source.stored_encryption(),
        SourceEncryption::Unknown,
        "the nested fixture has no authoritative encryption declaration",
    );
    let soc = original(&container, SOC)?;
    let tokens: Vec<_> = soc.split('\0').collect();
    let paths: Vec<_> = tokens
        .windows(2)
        .filter_map(|pair| match pair {
            [PROPERTY, value] => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(
        paths,
        [DECLARED_PATH],
        "one actual configured inventory path"
    );
    let [(first_path, _), _] = FILES;
    let mut occurrences = archive
        .entries()
        .iter()
        .filter(|entry| entry.path().eq_ignore_ascii_case(first_path));
    let first = occurrences
        .next()
        .ok_or("first actual directory occurrence")?;
    let second = occurrences
        .next()
        .ok_or("second actual directory occurrence")?;
    assert!(
        occurrences.next().is_none(),
        "exactly two path-equivalent files"
    );
    let mut sources = [(0, ""); 2];
    for ((entry, (path, item)), slot) in [first, second].into_iter().zip(FILES).zip(&mut sources) {
        assert_eq!(entry.path(), path, "original spelling and directory order");
        let payload: Value = serde_json::from_str(&original(archive, path)?)?;
        assert_eq!(
            payload.pointer("/Inventory/0/ID/ID/0"),
            Some(&json!(item)),
            "this physical file authors its own item"
        );
        *slot = (entry.index(), item);
    }
    Ok(sources)
}

fn assert_targets(catalog: &Value, row: &Value, originals: [(usize, &str); 2]) -> TestResult {
    let definitions = health::rows(catalog, "/definitions/shopInventories/rows")?;
    let targets = health::rows(row, "/physicalTargets/value")?;
    assert_eq!(targets.len(), 2, "both physical files are possible targets");
    for (index, _) in originals {
        let index = u64::try_from(index)?;
        let definition = definitions
            .iter()
            .find(|definition| definition.get("entryIndex").and_then(Value::as_u64) == Some(index))
            .ok_or("physical inventory definition at the actual directory position")?;
        let target = targets
            .iter()
            .find(|target| {
                target.pointer("/value/entryIndex").and_then(Value::as_u64) == Some(index)
            })
            .ok_or("canonical reference to the actual directory occurrence")?;
        assert_eq!(
            target["state"], "present",
            "the original target is readable"
        );
        assert_eq!(
            health::at(target, "/value/inputRef")?,
            health::at(definition, "/inputRef")?,
            "the reference selects the exact physical owner"
        );
        assert_eq!(
            target["containsEncryptedSource"], false,
            "the reference is sourced from directory identity"
        );
    }
    Ok(())
}

#[test]
fn equivalent_inventory_paths_are_ambiguous_without_changing_legacy_shop_links() -> TestResult {
    let (_dir, archive) = archive()?;
    let originals = assert_original(&archive)?;
    let [_, (_, legacy_item)] = originals;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let legacy = health::rows(&catalog, "/shops")?
        .iter()
        .find(|shop| shop["entityGuid"] == EXCHANGE)
        .ok_or("legacy exchange shop")?;
    assert_eq!(
        legacy["inventory"], "inv_commex",
        "legacy stem is preserved"
    );
    assert_eq!(
        legacy["itemGuids"],
        json!([legacy_item]),
        "compatibility retains the original last-directory selection"
    );
    assert_eq!(
        legacy["starmapObjectGuid"], "10000000-0000-4000-8000-000000000002",
        "the existing Hurston shop link survives"
    );
    let key = legacy["key"].as_str().ok_or("registry shop key")?;
    let rows = health::rows(&catalog, "/declarations/shopInventoryResolution/rows")?;
    let row = resolution_row(rows, key)?;
    let status = health::cell(row, "status", "present")?;
    assert_eq!(
        status["value"], "ambiguous",
        "one matching stem cannot identify either physical file uniquely"
    );
    let candidates = health::rows(row, "/candidates/value")?;
    let [candidate] = candidates else {
        return Err("one configured file candidate despite two physical files".into());
    };
    health::present(
        health::at(candidate, "/value")?,
        "sourcePath",
        &json!(DECLARED_PATH),
        None,
    )?;
    assert_targets(&catalog, row, originals)
}
