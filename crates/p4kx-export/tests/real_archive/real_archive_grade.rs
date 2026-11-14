// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The letter of an item grade in the P4KX catalog of a real Archive; runs with `--ignored`
//! when `P4KX_DATA_P4K` names one.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const LETTER_TYPES: [&str; 5] = ["Cooler", "PowerPlant", "QuantumDrive", "Radar", "Shield"];
const MIN_AGREEING_SHARE: f64 = 0.95;

fn letter_of(description: &str) -> Option<&str> {
    description
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("Grade:"))
        .and_then(|rest| rest.split_whitespace().next())
}

fn integer_of(letter: &str) -> Option<i64> {
    match letter {
        "A" => Some(1),
        "B" => Some(2),
        "C" => Some(3),
        "D" => Some(4),
        _ => None,
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_grade_letter_of_ship_components_follows_the_integer() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, None)?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let items = catalog["items"].as_array().ok_or("the items array")?;
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for item in items {
        let grade = &item["grade"];
        assert!(
            grade.is_null() || grade.as_i64().is_some_and(|value| (1..=7).contains(&value)),
            "grade is null or 1 to 7: {grade}"
        );
        let Some(kind) = item["type"]
            .as_str()
            .filter(|kind| LETTER_TYPES.contains(kind))
        else {
            continue;
        };
        let Some(expected) = item["description"]["en"]
            .as_str()
            .and_then(letter_of)
            .and_then(integer_of)
        else {
            continue;
        };
        let entry = counts.entry(kind).or_default();
        entry.0 += 1;
        entry.1 += usize::from(grade.as_i64() == Some(expected));
    }
    for kind in LETTER_TYPES {
        let (lettered, agreeing) = counts.get(kind).copied().ok_or("a lettered type")?;
        assert!(lettered > 0, "{kind} has items with a Grade line");
        #[expect(clippy::cast_precision_loss, reason = "counts far below 2^52")]
        let share = agreeing as f64 / lettered as f64;
        assert!(
            share >= MIN_AGREEING_SHARE,
            "{kind}: {agreeing} of {lettered} items agree with 1 = A to 4 = D"
        );
    }
    Ok(())
}
