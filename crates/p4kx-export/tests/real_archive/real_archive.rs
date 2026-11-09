// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Basetool catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogSummary, CraftingWarning, DECOMPRESSED_LIMIT, Kind, Language,
    UPLOAD_LIMIT, build_catalog, catalog_schema,
};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MIN_MANUFACTURERS: usize = 50;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn builds_a_valid_catalog_of_a_real_archive() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let validator = jsonschema::validator_for(&catalog_schema())?;
    let errors: Vec<String> = validator
        .iter_errors(&catalog)
        .take(5)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "valid against the schema: {errors:?}");

    let summary = built.summary();
    let manufacturers = summary
        .kind(Kind::Manufacturers)
        .ok_or("the manufacturers kind")?;
    assert!(
        manufacturers.count >= MIN_MANUFACTURERS,
        "plausible manufacturer count: {manufacturers:?}"
    );
    assert!(manufacturers.complete, "complete without a Filter");
    report_size(summary.size());
    assert!(
        summary.size() < DECOMPRESSED_LIMIT,
        "under the decompressed limit"
    );

    assert!(catalog.get("manufacturers").is_none(), "no legacy array");
    let list = catalog["definitions"]["manufacturers"]["rows"]
        .as_array()
        .ok_or("canonical manufacturers")?;
    let guids = list
        .iter()
        .map(|row| row["guid"]["value"].as_str().ok_or("manufacturer GUID"))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(guids.is_sorted(), "ordered by GUID");
    check_texts(summary, &catalog, list)?;
    crate::real_archive_texts::check_texts(&archive, &catalog, true)?;
    for row in list {
        let path = row["recordPath"]["value"]
            .as_str()
            .ok_or("manufacturer recordPath")?;
        let class = row["className"]["value"]
            .as_str()
            .ok_or("manufacturer className")?;
        assert!(!path.contains("_test_"), "no test leftovers");
        assert!(
            !class.is_empty() && !class.starts_with("SCItemManufacturer."),
            "class names without the type"
        );
    }
    Ok(())
}

const EMPTY_INVENTORY: &str = "inv_shopping_test_featuretest.json";

/// Returns whether `warning` is a known defect of the game data: the Build ships the shop
/// inventory file `inv_shopping_test_featuretest.json` with no content at all.
fn is_known_defect(warning: &CraftingWarning) -> bool {
    match warning {
        CraftingWarning::UnreadableShopInventories { count, details } => {
            *count == details.len()
                && details.iter().all(|detail| {
                    detail.to_ascii_lowercase().contains(EMPTY_INVENTORY)
                        && detail.ends_with("the file is empty")
                })
        }
        _ => false,
    }
}

/// Returns the warnings of `summary` that are not a known defect of the game data.
pub(crate) fn unexpected_warnings(summary: &CatalogSummary) -> Vec<&CraftingWarning> {
    summary
        .warnings()
        .iter()
        .filter(|warning| !is_known_defect(warning))
        .collect()
}

/// Returns whether every selected Record of `summary` is readable and its warnings are known
/// defects of the game data only.
pub(crate) fn is_complete_but_known_defects(summary: &CatalogSummary) -> bool {
    summary.failures().is_empty() && unexpected_warnings(summary).is_empty()
}

#[expect(
    clippy::print_stdout,
    reason = "the size is the measurement the upload limit is sized from"
)]
fn report_size(size: u64) {
    println!("catalog size: {size} bytes, upload limit {UPLOAD_LIMIT} bytes");
}

fn check_texts(summary: &CatalogSummary, catalog: &Value, list: &[Value]) -> TestResult {
    for code in ["RSI", "ANVL", "DRAK", "ACAS"] {
        let row = list
            .iter()
            .find(|row| row.pointer("/code/value").and_then(Value::as_str) == Some(code))
            .ok_or_else(|| format!("manufacturer {code}"))?;
        let name = crate::real_archive_texts::manufacturer_text(catalog, row, "name")?
            .ok_or("known manufacturer name")?;
        assert!(
            name.pointer("/key/value")
                .and_then(Value::as_str)
                .is_some_and(|key| key.starts_with('@')),
            "manufacturer {code} retains its authored key"
        );
        for language in ["en", "de"] {
            assert!(
                name.pointer(&format!("/translations/value/{language}/value"))
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.is_empty()),
                "manufacturer {code} has a {language} name"
            );
        }
    }
    for language in [Language::English, Language::German] {
        let texts = summary.text(language).ok_or("a text summary")?;
        assert!(texts.available, "the Archive holds {language}");
        assert!(
            texts.resolved > 0,
            "the {language} resolver counts the verified manufacturer names: {texts:?}"
        );
    }
    Ok(())
}
