// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, ExportHealthStatus, Kind};
use serde_json::{Value, json};

use crate::loot_nonfinite_v3_fixture::{self as fixture, Case, Checked, Encoding};

#[test]
fn finite_loot_numbers_have_a_valid_plain_and_compressed_source_control() -> Checked {
    for encoding in [Encoding::Stored, Encoding::Zstd] {
        let input = fixture::archive(encoding, Case::Finite)?;
        fixture::check_source(&input.archive, Case::Finite)?;
        let built = p4kx_export::build_catalog(&input.archive, &CatalogOptions::new(UNIX_EPOCH))?;
        assert_eq!(
            built.summary().export_health().status,
            ExportHealthStatus::Complete,
            "the finite stored and compressed positive controls remain complete"
        );
    }
    Ok(())
}

#[test]
fn plain_loot_nonfinite_occurrences_have_exact_emitted_coverage() -> Checked {
    check_complete(Encoding::Stored)
}

#[test]
fn compressed_loot_nonfinite_occurrences_have_exact_emitted_coverage() -> Checked {
    check_complete(Encoding::Zstd)
}

fn check_complete(encoding: Encoding) -> Checked {
    let input = fixture::archive(encoding, Case::Nonfinite)?;
    fixture::check_source(&input.archive, Case::Nonfinite)?;
    let built = p4kx_export::build_catalog(&input.archive, &CatalogOptions::new(UNIX_EPOCH))?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "exact raw coverage makes no nonfinite value disappear"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::LootPoolFilters)
            .map(|kind| kind.count),
        Some(1),
        "one actual physical Loot Record yields one definition row"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_numbers(&catalog)
}

#[test]
fn exact_numeric_coverage_does_not_waive_an_independent_unreadable_token() -> Checked {
    let input = fixture::archive(Encoding::Zstd, Case::Unreadable)?;
    fixture::check_source(&input.archive, Case::Unreadable)?;
    assert!(
        matches!(
            p4kx_export::build_catalog(&input.archive, &CatalogOptions::new(UNIX_EPOCH)),
            Err(CatalogError::IncompleteExport(_))
        ),
        "numeric receipts do not prove an unreadable Bool"
    );
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&input.archive, &options)?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Incomplete,
        "the actual unreadable Bool remains a blocking diagnostic cause"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_numbers(&catalog)?;
    let row = row(&catalog)?;
    let corrupt = follow(row, 6)?;
    assert_eq!(
        corrupt.pointer("/value/state"),
        Some(&json!("unreadable")),
        "the malformed original Bool has no successful semantic payload"
    );
    assert_eq!(
        corrupt.pointer("/value/value"),
        Some(&Value::Null),
        "unreadable value is explicitly null"
    );
    assert_eq!(
        corrupt.pointer("/originalError/type"),
        Some(&json!("invalidBool")),
        "the original typed failure survives independently of semantic null"
    );
    assert_eq!(
        corrupt.pointer("/originalError/byte/value"),
        Some(&json!(129)),
        "the complete original malformed byte survives"
    );
    Ok(())
}

fn check_numbers(catalog: &Value) -> Checked {
    let row = row(catalog)?;
    assert_eq!(
        row.get("values").and_then(Value::as_array).map(Vec::len),
        Some(10),
        "every original scalar/array/null/error occurrence, including duplicates"
    );
    for (field, kind, bits) in [
        (1, "f32", "7fc01234"),
        (2, "f64", "fff0000000000000"),
        (4, "f32", "7fc01234"),
        (5, "f32", "7fc01234"),
    ] {
        let node = follow(row, field)?;
        assert_number(node, kind, bits);
    }
    let samples = follow(row, 3)?
        .pointer("/value/value/elements")
        .and_then(Value::as_array)
        .ok_or("full original samples")?;
    assert_eq!(
        samples.len(),
        3,
        "all three actual source array slots survive"
    );
    for (reference, bits) in
        samples
            .iter()
            .zip(["7ff8000000000042", "fff0000000000000", "0000000000000000"])
    {
        assert_number(follow_ref(row, reference)?, "f64", bits);
    }
    assert_ne!(
        row.pointer("/fields/4/value/valueIndex"),
        row.pointer("/fields/5/value/valueIndex"),
        "equal same-name tokens are distinct original occurrences"
    );
    Ok(())
}

fn assert_number(node: &Value, kind: &str, bits: &str) {
    assert_eq!(
        node.pointer("/value/state"),
        Some(&json!("present")),
        "the exact numeric token remains present"
    );
    assert_eq!(
        node.pointer("/value/value/type"),
        Some(&json!("number")),
        "typed numeric payload"
    );
    assert_eq!(
        node.pointer("/value/value/value/type"),
        Some(&json!(kind)),
        "original numeric storage width"
    );
    assert_eq!(
        node.pointer("/value/value/value/rawBits"),
        Some(&json!(bits)),
        "exact original width and IEEE storage"
    );
    assert_eq!(
        node.get("originalError"),
        Some(&Value::Null),
        "a numeric source occurrence is not a parser error"
    );
}

fn row(catalog: &Value) -> Checked<&Value> {
    let rows = catalog
        .pointer("/definitions/lootPoolFilters/rows")
        .and_then(Value::as_array)
        .ok_or("canonical original Loot definitions")?;
    assert_eq!(rows.len(), 1, "the exact original physical numeric row");
    rows.first().ok_or_else(|| "original numeric row".into())
}

fn follow(row: &Value, field: usize) -> Checked<&Value> {
    let reference = row
        .get("fields")
        .and_then(Value::as_array)
        .and_then(|fields| fields.get(field))
        .and_then(|field| field.get("value"))
        .ok_or("original ordered field")?;
    follow_ref(row, reference)
}

fn follow_ref<'row>(row: &'row Value, reference: &Value) -> Checked<&'row Value> {
    let text = reference
        .get("valueIndex")
        .and_then(Value::as_str)
        .ok_or("decimal local reference")?;
    let index: u64 = text.parse()?;
    assert_eq!(text, index.to_string(), "canonical DecimalU64");
    row.get("values")
        .and_then(Value::as_array)
        .and_then(|values| values.get(usize::try_from(index).ok()?))
        .ok_or_else(|| "in-owner original occurrence".into())
}
