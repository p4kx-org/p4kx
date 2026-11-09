// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::super::check_coverage;
use super::{canonical_catalog, canonical_coverage};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn omit(catalog: &mut Value, state: &str, omit_parent: bool) -> TestResult<usize> {
    let rows = catalog
        .pointer_mut("/definitions/manufacturers/rows")
        .and_then(Value::as_array_mut)
        .ok_or("manufacturer rows")?;
    let count = rows.len();
    for row in rows {
        let localization = row.get_mut("localization").ok_or("Localization carrier")?;
        if omit_parent {
            localization["state"] = json!(state);
            localization["value"] = Value::Null;
        } else {
            for field in ["name", "description"] {
                let carrier = localization
                    .get_mut("value")
                    .and_then(|value| value.get_mut(field))
                    .ok_or("authored canonical child reference")?;
                carrier["state"] = json!(state);
                carrier["value"] = Value::Null;
            }
        }
    }
    Ok(count)
}

fn assert_drop(produced: &Value, state: &str, omit_parent: bool) -> TestResult {
    let mut omitted = produced.clone();
    let count = omit(&mut omitted, state, omit_parent)?;
    let (english, german) = canonical_coverage(&omitted)?;
    assert_eq!(
        english.total(),
        0,
        "the mutation removes all produced text references"
    );
    let mut failures = Vec::new();
    check_coverage(
        Kind::Manufacturers,
        &english,
        &german,
        true,
        count,
        &mut failures,
    );
    assert_eq!(
        failures.len(),
        1,
        "a positive baseline cannot become zero-key success"
    );
    failures.clear();
    check_coverage(
        Kind::Manufacturers,
        &english,
        &german,
        false,
        count,
        &mut failures,
    );
    assert!(
        failures.is_empty(),
        "a non-baselined source has no invented text floor"
    );
    Ok(())
}

#[test]
fn a_positive_manufacturer_baseline_cannot_skip_all_source_texts() -> TestResult {
    let produced = canonical_catalog()?;
    for (state, omit_parent) in [
        ("absent", true),
        ("null", true),
        ("absent", false),
        ("null", false),
    ] {
        assert_drop(&produced, state, omit_parent)?;
    }
    let mut empty = produced;
    empty["definitions"]["manufacturers"]["rows"] = json!([]);
    let (english, german) = canonical_coverage(&empty)?;
    let row_count = empty
        .pointer("/definitions/manufacturers/rows")
        .and_then(Value::as_array)
        .ok_or("zero-row family")?
        .len();
    let mut failures = Vec::new();
    check_coverage(
        Kind::Manufacturers,
        &english,
        &german,
        true,
        row_count,
        &mut failures,
    );
    assert!(
        failures.is_empty(),
        "an actual zero-row family has no invented text"
    );
    Ok(())
}
