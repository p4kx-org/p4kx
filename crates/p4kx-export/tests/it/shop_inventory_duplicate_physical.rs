// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::shops::{
    ShopRegistrySelection, read_shop, read_shop_with_provenance_mode,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, StarmapTraversalBudget};
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
    for (index, (name, item)) in FILES.into_iter().enumerate() {
        let alternatives = if index == 0 {
            vec![item.to_owned()]
        } else {
            vec![item.to_owned(), item.to_ascii_uppercase()]
        };
        let payload = json!({"Inventory": [{"ID": {"ID": alternatives}}]});
        let entry = TestEntry::stored(name, serde_json::to_vec(&payload)?);
        builder = builder.entry(if index == 0 { entry } else { entry.encrypted() });
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
    let built = p4kx_export::build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let legacy = health::rows(&catalog, "/shops")?
        .iter()
        .find(|shop| shop["entityGuid"]["value"] == EXCHANGE)
        .ok_or("legacy exchange shop")?;
    assert_eq!(
        legacy["inventory"]["value"], "inv_commex",
        "legacy stem is preserved"
    );
    assert_eq!(
        crate::catalog_shop_items::item_values(legacy)?,
        [legacy_item],
        "compatibility retains the original last-directory selection"
    );
    assert_eq!(
        legacy["starmapObjectGuid"]["value"], "10000000-0000-4000-8000-000000000002",
        "the existing Hurston shop link survives"
    );
    let key = legacy["key"]["value"].as_str().ok_or("registry shop key")?;
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
    assert_targets(&catalog, row, originals)?;
    assert_compatibility_origin(&catalog, legacy, originals)?;
    assert_physical_shop_scope(&archive, &catalog, legacy)?;
    super::resolution_sdk::assert_physical_duplicate_scope(&archive, &catalog, row, key)
}

fn assert_compatibility_origin(
    catalog: &Value,
    shop: &Value,
    [first, last]: [(usize, &str); 2],
) -> TestResult {
    let definitions = health::rows(catalog, "/definitions/shopInventories/rows")?;
    for ((index, _), expected) in [(first, false), (last, true)] {
        let index = u64::try_from(index)?;
        let definition = definitions
            .iter()
            .find(|entry| entry["entryIndex"].as_u64() == Some(index))
            .ok_or("actual physical inventory row")?;
        assert_eq!(
            health::at(definition, "/document/containsEncryptedSource")?,
            &json!(expected),
            "the controls differ only in which inventory payload is encrypted"
        );
        if expected {
            assert_last_file(catalog, shop, definition)?;
        }
    }
    Ok(())
}

fn assert_last_file(catalog: &Value, shop: &Value, definition: &Value) -> TestResult {
    let inventory = health::cell(shop, "inventory", "present")?;
    let items = health::cell(shop, "itemGuids", "present")?;
    for carrier in [inventory, items] {
        assert_eq!(
            carrier["containsEncryptedSource"], true,
            "the protected winning payload contributes to the selected inventory"
        );
        let physical_input = health::at(definition, "/inputRef")?;
        let uses = health::uses(catalog, carrier)?;
        assert!(
            uses.iter()
                .any(|used| used.get("source") == Some(physical_input)),
            "the winning file contributes directly to both compatibility values"
        );
    }
    let [item] = health::rows(items, "/value")? else {
        return Err("exactly one deduplicated GUID".into());
    };
    assert_eq!(
        item["containsEncryptedSource"], true,
        "the GUID is itself authored in the encrypted winning file"
    );
    let alternatives = health::rows(
        definition,
        "/document/value/fields/inventory/value/0/value/id/value/alternatives/value",
    )?;
    let actual = health::uses(catalog, item)?;
    assert_eq!(
        actual.len(),
        2,
        "both original alternatives contribute once"
    );
    for alternative in alternatives {
        let uses = health::uses(catalog, alternative)?;
        let [leaf] = uses.as_slice() else {
            return Err("one original JSON leaf".into());
        };
        assert!(
            actual.contains(leaf),
            "the actual original leaf contributes"
        );
    }
    Ok(())
}

fn assert_physical_shop_scope(archive: &Archive, catalog: &Value, normal: &Value) -> TestResult {
    let selection = registry_selection(archive, catalog, normal)?;
    let snapshot = read_shop_with_provenance_mode(
        archive,
        selection,
        StarmapTraversalBudget::default(),
        full_proof::MODE,
        None,
    )?;
    assert!(
        !snapshot.coverage.registry_complete,
        "the registry fixture includes an unrelated row with mismatched ID and name counts"
    );
    assert_eq!(
        snapshot.coverage.inventory_payloads, 1,
        "only the chosen last physical compatibility file was read"
    );
    let row = snapshot.row.ok_or("the exact admitted registry node")?;
    let scoped_wire = serde_json::to_value(&row)?;
    let scoped = json!({"provenance": snapshot.source_inventory});
    assert_eq!(
        health::with_origins(scoped_wire, &scoped)?,
        health::with_origins(normal.clone(), catalog)?,
        "normal and scoped Shop fields resolve to the same exact physical inputs"
    );
    assert_session_shop_scope(archive, selection, catalog, normal)?;
    let missing = ShopRegistrySelection::new(selection.entry_index, u32::MAX);
    let missing = read_shop(archive, missing, StarmapTraversalBudget::default(), None)?;
    assert!(
        missing.row.is_none(),
        "an unmatched physical node cannot select another row"
    );
    assert_eq!(
        missing.coverage.inventory_payloads, 0,
        "no unrelated inventory was read"
    );
    assert!(
        missing.coverage.names.is_none(),
        "no Starmap join was needed"
    );
    Ok(())
}

fn registry_selection(
    archive: &Archive,
    catalog: &Value,
    row: &Value,
) -> TestResult<ShopRegistrySelection> {
    let entry =
        p4kx_extraction::find_entry(archive, REGISTRY).ok_or("the original registry Entry")?;
    let key = health::cell(row, "key", "present")?;
    let uses = health::uses(catalog, key)?;
    let coordinate = uses
        .iter()
        .find_map(|used| {
            (used.pointer("/location/field")?.as_str()? == "@ID")
                .then(|| used.pointer("/location/indices/0"))
                .flatten()
        })
        .ok_or("the original ShopBackup node coordinate")?;
    let node = coordinate
        .as_str()
        .ok_or("the decimal XML occurrence")?
        .parse()?;
    Ok(ShopRegistrySelection::new(
        u32::try_from(entry.index())?,
        node,
    ))
}

fn assert_session_shop_scope(
    archive: &Archive,
    selection: ShopRegistrySelection,
    catalog: &Value,
    normal: &Value,
) -> TestResult {
    let budget = StarmapTraversalBudget::default();
    let session = full_proof::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let normal = health::with_origins(normal.clone(), catalog)?;
    for snapshot in [
        session.read_shop(selection, budget, None)?,
        parsed.read_shop(selection, budget, None)?,
    ] {
        assert!(
            snapshot.coverage.names.is_some(),
            "this session actually uses the reusable DCB to resolve a registry location"
        );
        let scoped_row = snapshot
            .row
            .ok_or("one selected Shop from the existing session")?;
        let scope = json!({"provenance": snapshot.source_inventory});
        assert_eq!(
            health::with_origins(serde_json::to_value(scoped_row)?, &scope)?,
            normal,
            "both reusable paths retain the canonical Shop and actual original uses"
        );
    }
    Ok(())
}
