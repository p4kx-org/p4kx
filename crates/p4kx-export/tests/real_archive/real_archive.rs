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

    let list = catalog["manufacturers"]
        .as_array()
        .ok_or("the manufacturers array")?;
    let guids: Vec<&str> = list.iter().filter_map(|m| m["guid"].as_str()).collect();
    assert!(guids.is_sorted(), "ordered by GUID");
    let codes: Vec<&str> = list.iter().filter_map(|m| m["code"].as_str()).collect();
    for code in ["RSI", "ANVL", "DRAK", "ACAS"] {
        assert!(codes.contains(&code), "manufacturer {code} is present");
        let named = list.iter().any(|m| {
            m["code"] == code
                && m["name"]["key"]
                    .as_str()
                    .is_some_and(|key| key.starts_with('@'))
        });
        assert!(named, "manufacturer {code} has a localisation key");
    }
    check_texts(summary, list)?;
    crate::real_archive_texts::check_texts(&archive, &catalog, true)?;
    assert!(
        list.iter()
            .filter_map(|m| m["recordPath"].as_str())
            .all(|path| !path.contains("_test_")),
        "no test leftovers"
    );
    assert!(
        list.iter()
            .filter_map(|m| m["className"].as_str())
            .all(|class| !class.is_empty() && !class.starts_with("SCItemManufacturer.")),
        "class names without the type"
    );
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

fn check_texts(summary: &CatalogSummary, list: &[Value]) -> TestResult {
    for code in ["RSI", "ANVL", "DRAK", "ACAS"] {
        let resolved = list.iter().any(|m| {
            m["code"] == code
                && ["/name/en", "/name/de"].into_iter().all(|text| {
                    m.pointer(text)
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.is_empty())
                })
        });
        assert!(
            resolved,
            "manufacturer {code} has an English and a German name"
        );
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
