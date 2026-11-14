// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use crate::health_origin_reference_source as source;
use crate::health_reference_origins as origins;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use serde_json::{Value, json};

#[test]
fn diagnostic_identities_distinguish_authored_guids_target_headers_and_original_pointer_words()
-> TestResult {
    let bytes = source::database();
    let words = source::original(&bytes)?;
    let input = Fixture::open(&bytes, Input::Encrypted, Vec::new())?;
    let filter = Filter::new("*/probe");
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..fixture::options()
    };
    let built = build_catalog_from(&input.archive, &input.entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let references = check::rows(&catalog, "/exportHealth/unresolvedReferences")?;
    let dangling = by_status(references, "dangling")?;
    let guid = check::present(
        dangling,
        "targetGuid",
        &json!(fixture::guid(source::MISSING)),
        Some(true),
    )?;
    own_body(&catalog, guid, 2, "$error.guid", "payload")?;
    let owner = check::present(
        dangling,
        "recordGuid",
        &json!(fixture::guid(source::OWNER)),
        Some(true),
    )?;
    check::header(&catalog, owner, 1, "guid", "payload", "archiveEntry")?;
    resolved_target(&catalog, references)?;
    pointer(&catalog, by_status(references, "invalidPointer")?, words)?;
    let twice = by_status(references, "ownedTwice")?;
    check::present(
        twice,
        "recordGuid",
        &json!(fixture::guid(source::OWNER)),
        Some(true),
    )?;
    example(&catalog)?;
    check::closure(&catalog)
}

fn resolved_target(catalog: &Value, references: &[Value]) -> TestResult {
    let resolved = by_status(references, "excludedByFilter")?;
    let guid = check::present(
        resolved,
        "targetGuid",
        &json!(fixture::guid(source::TARGET)),
        Some(true),
    )?;
    own_body(catalog, guid, 1, "$value", "payload")?;
    let index = check::present(resolved, "targetRecordIndex", &json!(0), Some(true))?;
    origins::exact(
        catalog,
        index,
        &[
            origins::header(0, "recordIndex", "selector"),
            origins::body(1, "$value", "selector"),
        ],
    )?;
    let path = check::present_text(
        catalog,
        resolved,
        "targetRecordPath",
        &json!("libs/foundry/records/scitemmanufacturer/target"),
        Some(true),
    )?;
    origins::exact(
        catalog,
        path,
        &[
            origins::header(0, "fileName", "conversion"),
            origins::body(1, "$value", "selector"),
        ],
    )?;
    Ok(())
}

fn by_status<'a>(rows: &'a [Value], status: &str) -> TestResult<&'a Value> {
    rows.iter()
        .find(|row| row.get("status").and_then(Value::as_str) == Some(status))
        .ok_or_else(|| format!("actual unresolved reference {status}").into())
}
fn own_body(catalog: &Value, cell: &Value, ordinal: u64, part: &str, role: &str) -> TestResult {
    origins::exact(catalog, cell, &[origins::body(ordinal, part, role)])
}
fn pointer(catalog: &Value, row: &Value, words: (u32, u32)) -> TestResult {
    let pointer = check::at(row, "/sourcePointer")?;
    for (field, expected, part) in [
        ("structIndex", words.0, "$error.structIndex"),
        ("instanceIndex", words.1, "$error.instanceIndex"),
    ] {
        let cell = check::present(pointer, field, &json!(expected), Some(true))?;
        own_body(catalog, cell, 6, part, "payload")?;
    }
    Ok(())
}
fn example(catalog: &Value) -> TestResult {
    let diagnostic = check::rows(catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| row.get("code") == Some(&json!("danglingReference")))
        .ok_or("actual dangling diagnostic")?;
    let example = check::rows(diagnostic, "/examples")?
        .first()
        .ok_or("actual original example")?;
    let guid = check::present(
        example,
        "targetGuid",
        &json!(fixture::guid(source::MISSING)),
        Some(true),
    )?;
    own_body(catalog, guid, 2, "$error.guid", "payload")?;
    check::present(
        example,
        "recordGuid",
        &json!(fixture::guid(source::OWNER)),
        Some(true),
    )?;
    check::present_text(catalog, example, "field", &json!("Dangling"), Some(true))?;
    check::present_text(
        catalog,
        example,
        "sourceType",
        &json!("SCItemManufacturer"),
        Some(true),
    )?;
    Ok(())
}
