// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read as _;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::Archive;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::{TestResult, shop};
use crate::health_origin_assertions as health;
use crate::shop_items_rest_fixture::{self, ITEM_A, ITEM_B};
use crate::shops_fixture::{EXCHANGE, FOOD};

#[path = "shop_inventory_normalized_id_domains.rs"]
mod id_domains;

const ORPHAN: &str = "Data\\Scripts\\ShopInventories\\Inv_ExactOrphan.json";
const PLACEHOLDER: &str = "Data\\Scripts\\ShopInventories\\Temp_Outpost_ShopInventory.json";
const PRECISE_PRICE: &str = "9007199254740993.2500";
const NEGATIVE_ZERO: &str = "-0.00e+00";
const FOOD_SHOP_ID: &str = concat!(
    "4a269932-183e-362d-aacc-188e7debbea9,",
    "4ce1f681-b479-5269-0536-3d7f02fdccaf,",
    "41000000-0000-4000-8000-000000000004,",
    "4a000000-0000-4000-8000-0000000000f1"
);

fn orphan() -> String {
    let repeated = format!(
        r#"{{"ID":{{"ID":["{ITEM_B}","{ITEM_A}","{ITEM_B}"]}},"BuyPrice":{PRECISE_PRICE},"CurrentInventory":0,"RentalOfferings":[]}}"#
    );
    let last = format!(
        r#"{{"ID":{{"ID":["{ITEM_A}"]}},"BuyPrice":{NEGATIVE_ZERO},"SellPrice":null,"RentalOfferings":[{{"FutureVariant":{{"Exact":1.2300e+12,"Note":"authored note"}}}}]}}"#
    );
    format!(
        r#"{{"ShopID":"{FOOD_SHOP_ID}","Collection":{{"Inventory":[{repeated},{repeated},{last}]}}}}"#
    )
}

fn original(archive: &Archive, path: &str, encrypted: bool) -> Result<String, Box<dyn Error>> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("physical inventory Entry")?;
    assert_eq!(
        entry.is_encrypted(),
        encrypted,
        "{path} physical encryption"
    );
    let mut decoded = String::new();
    archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_string(&mut decoded)?;
    Ok(decoded)
}

fn assert_original_inventories(archive: &Archive, authored: &str) -> TestResult {
    let source = original(archive, ORPHAN, true)?;
    assert_eq!(source, authored, "the real Archive holds every exact token");
    let parsed: Value = serde_json::from_str(&source)?;
    assert_eq!(
        health::at(&parsed, "/ShopID")?,
        &json!(FOOD_SHOP_ID),
        "authored ShopID evidence"
    );
    let [first, second, last] = health::rows(&parsed, "/Collection/Inventory")? else {
        return Err("three physical inventory line occurrences".into());
    };
    assert_eq!(first, second, "the two equal lines occupy distinct slots");
    assert_eq!(
        health::at(first, "/ID/ID")?,
        &json!([ITEM_B, ITEM_A, ITEM_B]),
        "the original alternatives retain order and duplicates"
    );
    assert_eq!(
        source.matches(PRECISE_PRICE).count(),
        2,
        "both physical price tokens have all original digits"
    );
    assert_eq!(
        source.matches(NEGATIVE_ZERO).count(),
        1,
        "the separate zero has an authored sign, scale and exponent"
    );
    assert!(first.get("SellPrice").is_none(), "absent price");
    assert!(
        health::at(last, "/SellPrice")?.is_null(),
        "explicitly null price"
    );
    assert_eq!(
        health::at(first, "/CurrentInventory")?,
        &json!(0),
        "stored stock zero"
    );

    let placeholder = original(archive, PLACEHOLDER, false)?;
    let parsed: Value = serde_json::from_str(&placeholder)?;
    assert_eq!(
        health::at(&parsed, "/Inventory/0/ID/ID/0")?,
        &json!(ITEM_A),
        "the physical placeholder uses the other inventory shape"
    );
    Ok(())
}

#[test]
fn physical_shop_inventories_include_orphans_and_placeholders() -> TestResult {
    let authored = orphan();
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    shop_items_rest_fixture::builder()
        .entry(TestEntry::stored(ORPHAN, authored.as_bytes()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    assert_original_inventories(&archive, &authored)?;

    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let food = shop(&catalog, FOOD)?;
    assert_eq!(
        food["key"], FOOD_SHOP_ID,
        "the orphan names this exact shop key"
    );
    assert_eq!(
        food["inventory"], "inv_noodle",
        "the real prefab controls selection independently of matching ShopID"
    );
    assert!(
        shop(&catalog, EXCHANGE)?["itemGuids"].is_null(),
        "a placeholder remains outside compatibility itemGuids"
    );
    assert_eq!(
        built.summary().shops().inventory_placeholder,
        1,
        "the fixture actually resolves the exchange as a placeholder"
    );

    let rows = health::rows(&catalog, "/definitions/shopInventories/rows")?;
    assert_eq!(
        rows.len(),
        5,
        "emit each of the five physical inventory files, including the orphan and placeholder"
    );
    assert_eq!(
        health::at(&catalog, "/definitions/shopInventories/meaning")?,
        &json!("offlineDesignTime"),
        "the complete dataset identifies the values as offline"
    );
    assert_orphan(&catalog, file(rows, ORPHAN)?, &authored)?;
    assert_placeholder(file(rows, PLACEHOLDER)?)
}

fn file<'catalog>(rows: &'catalog [Value], path: &str) -> Result<&'catalog Value, Box<dyn Error>> {
    rows.iter()
        .find(|row| row.pointer("/sourcePath/value").and_then(Value::as_str) == Some(path))
        .ok_or_else(|| format!("missing physical inventory {path}").into())
}

fn referenced<'document>(
    document: &'document Value,
    cell: &Value,
) -> Result<&'document Value, Box<dyn Error>> {
    let index: usize = cell
        .get("value")
        .and_then(Value::as_str)
        .ok_or("present typed arena reference")?
        .parse()?;
    health::rows(document, "/source/values")?
        .get(index)
        .ok_or_else(|| "original arena node".into())
}

fn assert_orphan(catalog: &Value, row: &Value, authored: &str) -> TestResult {
    health::present(row, "sourcePath", &json!(ORPHAN), Some(false))?;
    health::present(
        row,
        "normalizedPath",
        &json!(ORPHAN.to_ascii_lowercase()),
        Some(false),
    )?;
    let body = health::cell(row, "document", "present")?;
    assert_eq!(
        body["containsEncryptedSource"], true,
        "encrypted document content"
    );
    let document = health::at(body, "/value")?;
    let shop_id = health::at(document, "/fields/shopId")?;
    assert_eq!(
        health::at(referenced(document, shop_id)?, "/value/value/type")?,
        &json!("public"),
        "the actual authored ShopID is technical"
    );
    assert_eq!(
        health::at(referenced(document, shop_id)?, "/value/value/text")?,
        &json!(FOOD_SHOP_ID),
        "exact authored ShopID stays in the physical file"
    );
    let rows = health::rows(document, "/fields/collection/value/inventory/value")?;
    let [first, second, last] = rows else {
        return Err("three distinct authored line occurrences".into());
    };
    assert_ne!(
        health::at(first, "/value/node")?,
        health::at(second, "/value/node")?,
        "equal lines keep separate nodes"
    );
    assert_prices(catalog, row, document, [first, second, last], authored)?;
    assert_line_fields(document, first, last)
}

fn assert_prices(
    catalog: &Value,
    row: &Value,
    document: &Value,
    lines: [&Value; 3],
    authored: &str,
) -> TestResult {
    let uses = health::rows(catalog, "/provenance/uses")?;
    let input = health::at(row, "/inputRef")?;
    let locations = authored
        .match_indices(PRECISE_PRICE)
        .chain(authored.match_indices(NEGATIVE_ZERO));
    for (line, (start, token)) in lines.into_iter().zip(locations) {
        let price = health::at(line, "/value/buyPrice")?;
        assert_eq!(
            price["containsEncryptedSource"], true,
            "independent price marker"
        );
        assert_eq!(
            price["sourceSetComplete"], true,
            "complete actual price contributions"
        );
        assert_eq!(
            health::at(referenced(document, price)?, "/value/token")?,
            &json!(token),
            "source number retains the exact original decimal spelling"
        );
        let offsets = json!([start.to_string(), token.len().to_string()]);
        let references = health::rows(price, "/sourceUsesRefs")?;
        let matching = uses
            .iter()
            .filter(|used| {
                used.get("id").is_some_and(|id| references.contains(id))
                    && used.get("source") == Some(input)
                    && used.get("role").and_then(Value::as_str) == Some("payload")
                    && used.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("archiveJson")
                    && used.pointer("/location/indices") == Some(&offsets)
            })
            .count();
        assert_eq!(
            matching, 1,
            "the actual price slot contributes exactly once"
        );
    }
    Ok(())
}

fn assert_line_fields(document: &Value, first: &Value, last: &Value) -> TestResult {
    let first = health::at(first, "/value")?;
    let last = health::at(last, "/value")?;
    let alternatives = health::rows(first, "/id/value/alternatives/value")?;
    assert_eq!(alternatives.len(), 3, "three actual ordered ID slots");
    for (alternative, original) in alternatives.iter().zip([ITEM_B, ITEM_A, ITEM_B]) {
        assert_eq!(
            health::at(referenced(document, alternative)?, "/value/value/text")?,
            &json!(original),
            "each authored GUID alternative survives in order"
        );
    }
    health::cell(first, "sellPrice", "absent")?;
    health::cell(last, "sellPrice", "null")?;
    let stock = health::cell(first, "currentInventory", "present")?;
    assert_eq!(
        health::at(referenced(document, stock)?, "/value/token")?,
        &json!("0"),
        "stock zero is a value"
    );
    let rental = health::cell(first, "rentalOfferings", "present")?;
    assert_eq!(
        health::at(referenced(document, rental)?, "/value/elements")?,
        &json!([]),
        "present empty rental offerings are not absent"
    );
    let rental = health::cell(last, "rentalOfferings", "present")?;
    assert_unknown_rental(document, referenced(document, rental)?)
}

fn assert_unknown_rental(document: &Value, root: &Value) -> TestResult {
    let index = health::at(root, "/value/elements/0")?;
    let offering = referenced(document, &json!({"value": index}))?;
    let member = health::at(offering, "/value/members/0")?;
    assert_eq!(
        health::at(member, "/name/value/text")?,
        &json!("FutureVariant"),
        "the unknown authored variant name remains available"
    );
    let variant = referenced(document, member)?;
    let exact = health::at(variant, "/value/members/0")?;
    assert_eq!(
        health::at(referenced(document, exact)?, "/value/token")?,
        &json!("1.2300e+12"),
        "the unfamiliar nested rental number remains exact"
    );
    let note = health::at(variant, "/value/members/1")?;
    let note = health::at(referenced(document, note)?, "/value/value")?;
    assert_eq!(
        note,
        &json!({"type":"gameText","text":"authored note"}),
        "the actual unclassified authored text uses the protected domain"
    );
    Ok(())
}

fn assert_placeholder(row: &Value) -> TestResult {
    health::present(row, "sourcePath", &json!(PLACEHOLDER), Some(false))?;
    let body = health::cell(row, "document", "present")?;
    assert_eq!(
        body["containsEncryptedSource"], false,
        "plain placeholder payload"
    );
    let fields = health::at(body, "/value/fields")?;
    health::cell(fields, "collection", "absent")?;
    assert_eq!(
        health::rows(fields, "/inventory/value")?.len(),
        1,
        "top-level inventory remains available for a placeholder"
    );
    Ok(())
}
