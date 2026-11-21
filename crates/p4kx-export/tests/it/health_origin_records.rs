// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use p4kx_export::{CatalogError, Compression, Kind, write_catalog_from};
use p4kx_extraction::DataForgeError;
use serde_json::{Value, json};
use std::cell::Cell;

#[test]
fn failed_record_headers_keep_their_original_plain_encrypted_and_standalone_origins() -> TestResult
{
    let bytes = fixture::broken_lists(1, false);
    let mut inputs = Vec::new();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let source = Fixture::open(&bytes, input, Vec::new())?;
        original(&source)?;
        inputs.push((source, input));
    }
    for (source, input) in inputs {
        refused_before_creation(&source)?;
        let (built, catalog) = source.diagnostic()?;
        let health = built.summary().export_health();
        assert_eq!(
            health
                .kind(Kind::BlueprintCategoryLists)
                .ok_or("list health")?
                .written_rows,
            0,
            "the strict original failure omits its row"
        );
        let failed = check::rows(&catalog, "/exportHealth/failedRecords")?
            .first()
            .ok_or("actual failed Record")?;
        retained_headers(&catalog, failed, input)?;
        let scoped = check::rows(&catalog, "/exportHealth/kinds")?
            .iter()
            .find(|row| row.get("kind") == Some(&json!("blueprintCategoryLists")))
            .ok_or("scoped list health")?;
        assert_eq!(
            check::at(scoped, "/failedRecords/0")?,
            failed,
            "global and scoped views preserve the same physical source facts"
        );
        check::closure(&catalog)?;
    }
    Ok(())
}

fn retained_headers(catalog: &Value, failed: &Value, input: Input) -> TestResult {
    let kind = if matches!(input, Input::Standalone) {
        "file"
    } else {
        "archiveEntry"
    };
    let guid = check::present(
        failed,
        "guid",
        &json!(fixture::guid(fixture::LIST)),
        input.marker(),
    )?;
    check::header(catalog, guid, 0, "guid", "payload", kind)?;
    let path = check::present_text(
        catalog,
        failed,
        "recordPath",
        &json!("synthetic/health/List0"),
        input.marker(),
    )?;
    check::header(catalog, path, 0, "fileName", "conversion", kind)?;
    let index = check::present(failed, "recordIndex", &json!(0), input.marker())?;
    check::header(catalog, index, 0, "recordIndex", "selector", kind)?;
    let source_type = check::present_text(
        catalog,
        failed,
        "sourceType",
        &json!(fixture::LIST_TYPE),
        input.marker(),
    )?;
    check::header(catalog, source_type, 0, "sourceType", "selector", kind)?;
    Ok(())
}

fn original(source: &Fixture) -> TestResult {
    let forge = source.entry.open()?;
    let record = forge.record(0).ok_or("original failed Record")?;
    assert_eq!(record.index(), 0, "actual physical index");
    assert_eq!(
        record.guid().to_string(),
        fixture::guid(fixture::LIST),
        "actual source GUID"
    );
    assert_eq!(
        record.stored_file_name()?,
        "synthetic/health/List0.xml",
        "actual stored file name"
    );
    assert_eq!(
        record.type_name()?,
        fixture::LIST_TYPE,
        "actual source type before tree failure"
    );
    assert!(
        matches!(
            record.tree(),
            Err(DataForgeError::StructLayout {
                declared: 9,
                fields: 8,
                ..
            })
        ),
        "the original reader proves the intended strict failure"
    );
    Ok(())
}

fn refused_before_creation(source: &Fixture) -> TestResult {
    let created = Cell::new(false);
    let error = write_catalog_from(
        &source.archive,
        &source.entry,
        &fixture::options(),
        Compression::None,
        || {
            created.set(true);
            Ok(Vec::new())
        },
    )
    .err()
    .ok_or("normal publication must refuse the actual source failure")?;
    let CatalogError::IncompleteExport(refusal) = error else {
        return Err("normal failure must stay typed IncompleteExport".into());
    };
    let health = refusal.health();
    assert!(
        !created.get(),
        "normal refusal happens before a sink exists"
    );
    assert_eq!(
        health.failed_records().len(),
        1,
        "the actual refused report retains its failed subject"
    );
    Ok(())
}
