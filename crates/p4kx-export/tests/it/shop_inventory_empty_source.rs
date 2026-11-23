// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, CraftingWarning, build_catalog,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestElement, TestEntry, TestStruct};
use serde_json::{Value, json};

use super::{PREFAB, REGISTRY, TestResult, key};
use crate::health_origin_assertions as proof;

const INVENTORY: &str = "Data\\Scripts\\ShopInventories\\Inv_EmptySourceFixture.json";
const DEFINITIONS: &str = "/definitions/shopInventories/rows";
const RESOLUTIONS: &str = "/declarations/shopInventoryResolution/rows";

#[test]
fn a_fully_read_zero_byte_shop_source_publishes_with_the_legacy_warning() -> TestResult {
    for encrypted in [false, true] {
        let entry = TestEntry::stored(INVENTORY, Vec::new());
        let entry = if encrypted { entry.encrypted() } else { entry };
        let (_folder, archive) = fixture(Some(entry))?;
        let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
        assert!(
            built.summary().export_health().is_complete(),
            "verified empty source is complete"
        );
        let counts = built.summary().shops();
        assert_eq!(
            (
                counts.inventory_unreadable,
                counts.inventories_unreadable,
                counts.inventory_empty_source,
                counts.inventories_empty_source
            ),
            (1, 1, 1, 1),
            "legacy counts and the exact empty-source subset agree"
        );
        let details = built
            .summary()
            .warnings()
            .iter()
            .find_map(|warning| match warning {
                CraftingWarning::UnreadableShopInventories { count: 1, details } => Some(details),
                _ => None,
            })
            .ok_or("the legacy empty-file warning remains")?;
        assert!(
            matches!(details.as_slice(), [detail] if detail.ends_with(": the file is empty")),
            "the original warning reason remains"
        );
        let catalog = serde_json::from_slice(built.json())?;
        verify_empty_source(&catalog, encrypted)?;
        let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
        validator
            .validate(&catalog)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[test]
fn an_authored_empty_array_is_a_resolved_inventory() -> TestResult {
    let (_folder, archive) = fixture(Some(TestEntry::stored(INVENTORY, r#"{"Inventory":[]}"#)))?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog = serde_json::from_slice(built.json())?;
    let (shop, resolution) = one_shop(&catalog)?;
    assert_eq!(
        proof::at(shop, "/itemGuids/value")?,
        &json!([]),
        "the original empty list is present"
    );
    assert_eq!(
        proof::at(resolution, "/status/value")?,
        "resolved",
        "a valid JSON document yields an ordinary resolved list"
    );
    assert_eq!(
        built.summary().shops().inventory_empty_source,
        0,
        "no empty physical source"
    );
    Ok(())
}

#[test]
fn invalid_json_failed_reads_and_missing_targets_still_block_normal_publication() -> TestResult {
    let cases = [
        (
            Some(TestEntry::stored(INVENTORY, " \r\n\t")),
            "unreadable",
            Some("invalid"),
        ),
        (
            Some(TestEntry::stored(INVENTORY, r#"{"Inventory":["#)),
            "unreadable",
            Some("invalid"),
        ),
        (
            Some(TestEntry::stored(INVENTORY, "x").with_uncompressed_size(0)),
            "unreadable",
            Some("unreadable"),
        ),
        (
            Some(TestEntry::stored(INVENTORY, Vec::new()).with_crc32c(7)),
            "unreadable",
            Some("unreadable"),
        ),
        (None, "missing", None),
    ];
    for (entry, expected_status, document_state) in cases {
        let (_folder, archive) = fixture(entry)?;
        let mut options = CatalogOptions::new(UNIX_EPOCH);
        assert!(
            matches!(
                build_catalog(&archive, &options),
                Err(CatalogError::IncompleteExport(_))
            ),
            "an unusable or unavailable source still fails normal publication"
        );
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog(&archive, &options)?;
        let catalog = serde_json::from_slice(built.json())?;
        let (shop, resolution) = one_shop(&catalog)?;
        assert_no_list(
            shop,
            if expected_status == "missing" {
                "absent"
            } else {
                "unreadable"
            },
        )?;
        assert_eq!(
            proof::at(resolution, "/status/value")?,
            expected_status,
            "a failure keeps its original resolution state"
        );
        assert_definition_state(&catalog, document_state)?;
    }
    Ok(())
}

fn verify_empty_source(catalog: &Value, encrypted: bool) -> TestResult {
    let (shop, resolution) = one_shop(catalog)?;
    assert_no_list(shop, "absent")?;
    let [definition] = proof::rows(catalog, DEFINITIONS)? else {
        return Err("the physical empty source must remain in definitions".into());
    };
    let document = proof::cell(definition, "document", "absent")?;
    let status = proof::present(resolution, "status", &json!("emptySource"), Some(encrypted))?;
    assert_eq!(
        proof::at(document, "/containsEncryptedSource")?,
        &json!(encrypted),
        "the absence itself has the original physical payload marker"
    );
    assert_eq!(
        proof::at(document, "/sourceSetComplete")?,
        true,
        "the physical read completed"
    );
    let [target] = proof::rows(resolution, "/physicalTargets/value")? else {
        return Err("one exact physical target for the empty-source resolution".into());
    };
    assert_eq!(
        proof::at(target, "/value/entryIndex")?,
        proof::at(definition, "/entryIndex")?,
        "the selected declaration points to the same physical Entry"
    );
    let uses = proof::uses(catalog, document)?;
    let [payload] = uses.as_slice() else {
        return Err("only the actual empty payload contributes to the document's absence".into());
    };
    assert_eq!(
        proof::at(payload, "/location/namespace")?,
        "archiveEntryRead",
        "complete Entry payload"
    );
    assert_eq!(
        proof::at(payload, "/location/field")?,
        "content",
        "the original payload receipt"
    );
    assert_eq!(
        proof::at(payload, "/source")?,
        proof::at(definition, "/inputRef")?,
        "exact physical source"
    );
    let status_uses = proof::uses(catalog, status)?;
    assert!(
        status_uses.contains(payload),
        "the outcome cites the same measured payload"
    );
    assert!(
        status_uses
            .iter()
            .any(|usage| usage.pointer("/location/field") == Some(&json!("@offlineInventoryJSON"))),
        "the actual shop declaration also contributes to the outcome"
    );
    proof::closure(catalog)
}

fn one_shop(catalog: &Value) -> TestResult<(&Value, &Value)> {
    let [shop] = proof::rows(catalog, "/shops")? else {
        return Err("one selected Shop".into());
    };
    let [resolution] = proof::rows(catalog, RESOLUTIONS)? else {
        return Err("one original Inventory resolution".into());
    };
    Ok((shop, resolution))
}

fn assert_no_list(shop: &Value, state: &str) -> TestResult {
    proof::cell(shop, "inventory", state)?;
    proof::cell(shop, "itemGuids", state)?;
    assert!(
        proof::at(shop, "/inventory/value")?.is_null(),
        "no compatibility inventory"
    );
    assert!(
        proof::at(shop, "/itemGuids/value")?.is_null(),
        "no authored item list to publish"
    );
    Ok(())
}

fn assert_definition_state(catalog: &Value, state: Option<&str>) -> TestResult {
    match (state, proof::rows(catalog, DEFINITIONS)?) {
        (Some(expected), [definition]) => {
            proof::cell(definition, "document", expected)?;
        }
        (None, []) => {}
        _ => return Err("present failures retain identity and missing sources have no row".into()),
    }
    Ok(())
}

fn fixture(offline_inventory: Option<TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureEmpty"));
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(PREFAB, prefab()));
    if let Some(inventory) = offline_inventory {
        archive = archive.entry(inventory);
    }
    archive.write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn registry() -> String {
    let shop = TestElement::new("ShopBackup")
        .attribute("ID", key(0))
        .attribute("SuperName", "Root,FixtureShop");
    let layer = TestElement::new("LayerBackup")
        .attribute("PlatformName", "StantonSystem")
        .child(TestElement::new("ShopBackups").child(shop));
    TestElement::new("BackupData")
        .child(TestElement::new("LayerBackups").child(layer))
        .to_xml()
}

fn prefab() -> String {
    let component = TestElement::new("EntityComponentShop").attribute(
        "offlineInventoryJSON",
        "Scripts/ShopInventories/Inv_EmptySourceFixture.json",
    );
    let entity = TestElement::new("Object")
        .attribute("Type", "Entity")
        .attribute("Id", "{00004000-0000-4A00-0000-000000000080}")
        .child(TestElement::new("PropertiesDataCore").child(component));
    TestElement::new("PrefabsLibrary")
        .child(TestElement::new("Prefab").child(TestElement::new("Objects").child(entity)))
        .to_xml()
}
