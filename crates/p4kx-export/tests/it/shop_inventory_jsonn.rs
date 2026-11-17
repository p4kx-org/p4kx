// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{TestChunkFile, TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{
    CONTAINER, INVENTORIES, METADATA, REGISTRY, SOC, TestResult, entity, key, registry,
    resolution_row,
};
use crate::health_origin_assertions as proof;
use crate::starmap_fixture;

const ORIGINAL: &str = "Scripts/ShopInventories/Inventory.jsonn";
const TARGET: &str = "Inventory.json";
const JSON: &str = "{\"Inventory\":[]}";

#[test]
fn original_structured_jsonn_selects_the_only_supported_json_and_keeps_both_sources() -> TestResult
{
    let catalog = catalog("offlineInventoryJSON", ORIGINAL, JSON, &[])?;
    let row = first_row(&catalog)?;
    assert_eq!(
        proof::at(row, "/status/value")?,
        "resolved",
        "canonical JSON is readable"
    );
    let [candidate] = proof::rows(row, "/candidates/value")? else {
        return Err("one actual inventory declaration".into());
    };
    let original = proof::at(candidate, "/value/sourcePath")?;
    let stem = proof::at(candidate, "/value/stem")?;
    assert_eq!(
        proof::at(original, "/value")?,
        ORIGINAL,
        "unmodified source spelling"
    );
    assert_eq!(
        proof::at(stem, "/value")?,
        "inventory",
        "verified canonical target stem"
    );
    assert!(
        proof::uses(&catalog, original)?.iter().any(|item| {
            item.pointer("/location/field") == Some(&json!("offlineInventoryJSON"))
                && item.get("role") == Some(&json!("payload"))
        }),
        "the original value is the structured SOC payload"
    );
    assert!(
        proof::uses(&catalog, stem)?.iter().any(|item| {
            item.pointer("/location/type") == Some(&json!("metadata"))
                && item.pointer("/location/field") == Some(&json!("path"))
                && item.get("role") == Some(&json!("selector"))
        }),
        "target directory evidence selects this canonical stem"
    );
    let [target] = proof::rows(row, "/physicalTargets/value")? else {
        return Err("one actual physical Inventory Entry".into());
    };
    let targets = proof::rows(&catalog, "/definitions/shopInventories/rows")?;
    let chosen = targets
        .first()
        .ok_or("the canonical Inventory definition")?;
    assert_eq!(
        proof::at(target, "/value/entryIndex")?,
        proof::at(chosen, "/entryIndex")?,
        "the selection resolves the physical Entry the canonical reader used"
    );
    proof::closure(&catalog)
}

#[test]
fn reserved_correction_guards_keep_unrelated_or_ambiguous_paths_rejected() -> TestResult {
    for (attribute, authored, extra) in [
        ("offlineInventory", ORIGINAL, None),
        ("offlineInventoryJSON", "Other/Inventory.jsonn", None),
        (
            "offlineInventoryJSON",
            "Scripts/ShopInventories/Inventory.csv",
            None,
        ),
        ("offlineInventoryJSON", ORIGINAL, Some("Inventory.jsonn")),
        ("offlineInventoryJSON", ORIGINAL, Some("Inventory.xml")),
        ("offlineInventoryJSON", ORIGINAL, Some("INVENTORY.JSON")),
    ] {
        let extras: Vec<_> = extra.into_iter().collect();
        let catalog = catalog(attribute, authored, JSON, &extras)?;
        let row = first_row(&catalog)?;
        let candidates = proof::rows(row, "/candidates/value")?;
        let rejected = candidates
            .iter()
            .find(|candidate| candidate.pointer("/value/type") == Some(&json!("rejected")))
            .ok_or("the original rejected declaration")?;
        assert_eq!(
            proof::at(rejected, "/value/sourcePath/value")?,
            authored,
            "every rejected path retains the original value"
        );
        assert!(
            proof::rows(row, "/physicalTargets/value")?.is_empty(),
            "no guessed file selected"
        );
    }
    Ok(())
}

#[test]
fn corrected_path_still_requires_valid_json_from_the_selected_physical_entry() -> TestResult {
    let catalog = catalog("offlineInventoryJSON", ORIGINAL, "{invalid", &[])?;
    let row = first_row(&catalog)?;
    assert_eq!(
        proof::at(row, "/status/value")?,
        "unreadable",
        "the ordinary JSON reader owns validity"
    );
    assert_eq!(
        proof::rows(row, "/physicalTargets/value")?.len(),
        1,
        "the failed physical target stays identifiable"
    );
    Ok(())
}

fn first_row(catalog: &Value) -> TestResult<&Value> {
    let rows = proof::rows(catalog, "/declarations/shopInventoryResolution/rows")?;
    resolution_row(rows, &key(0))
}

fn catalog(attribute: &str, source: &str, json: &str, extra: &[&str]) -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let mut builder = starmap_fixture::builder()
        .entry(TestEntry::stored(REGISTRY, registry()))
        .entry(TestEntry::stored(CONTAINER, container(attribute, source)))
        .entry(TestEntry::stored(format!("{INVENTORIES}{TARGET}"), json));
    for name in extra {
        builder = builder.entry(TestEntry::stored(format!("{INVENTORIES}{name}"), JSON));
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(std::time::UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(catalog.json())?)
}

fn container(attribute: &str, source: &str) -> Vec<u8> {
    let guid = entity(0);
    let metadata = TestElement::new("EntityComponentShop")
        .child(TestElement::new("Entity").attribute("CryGUID", &guid))
        .to_xml();
    let xml = TestElement::new("Entities").child(
        TestElement::new("Entity")
            .attribute("EntityCryGUID", guid)
            .child(TestElement::new("EntityComponentShop").attribute(attribute, source)),
    );
    let soc = TestChunkFile::crch()
        .chunk(0x0004, 3, xml.to_xml().into_bytes())
        .bytes();
    TestContainer::new()
        .entry(TestEntry::stored(METADATA, metadata))
        .entry(TestEntry::stored(SOC, soc))
        .build()
}
