// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use p4kx_export::{CatalogError, CatalogFile, Compression, write_catalog_from};
use serde_json::{Value, json};
use std::cell::Cell;

#[test]
fn normal_refusal_exposes_the_same_retained_identity_facts_without_creating_output() -> TestResult {
    let source = Fixture::open(
        &fixture::broken_lists(1, false),
        Input::Encrypted,
        Vec::new(),
    )?;
    let forge = source.entry.open()?;
    let original = forge.record(0).ok_or("original failing subject")?;
    assert!(
        original.tree().is_err(),
        "the original source genuinely fails before projection"
    );
    let normal = refused_report(&source)?;
    let row = check::rows(&normal, "/failedRecords")?
        .first()
        .ok_or("actual error health subject")?;
    let normal_guid = check::present(row, "guid", &json!(original.guid().to_string()), Some(true))?;
    assert!(
        !check::at(normal_guid, "/sourceUsesRefs")?
            .as_array()
            .ok_or("actual error source references")?
            .is_empty(),
        "normal refusal preserves actual original uses instead of dropping them"
    );
    let (_, catalog) = source.diagnostic()?;
    let diagnostic = check::rows(&catalog, "/exportHealth/failedRecords")?
        .first()
        .ok_or("actual diagnostic subject")?;
    same_identity(
        &normal,
        row,
        check::at(&catalog, "/exportHealth")?,
        diagnostic,
    )?;
    assert_eq!(
        normal.get("publicationMode"),
        Some(&json!("normal")),
        "the refused report keeps its actual mode"
    );
    assert_eq!(
        check::at(&catalog, "/exportHealth/publicationMode")?,
        &json!("diagnosticPartial"),
        "diagnostic publication remains explicit"
    );
    check::closure(&catalog)
}

fn refused_report(source: &Fixture) -> TestResult<Value> {
    let output = source.folder.path().join("previous.json");
    std::fs::write(&output, b"previous complete catalog")?;
    let created = Cell::new(false);
    let result = write_catalog_from(
        &source.archive,
        &source.entry,
        &fixture::options(),
        Compression::None,
        || {
            created.set(true);
            CatalogFile::create(&output)
        },
    );
    let CatalogError::IncompleteExport(refusal) = result.err().ok_or("actual normal refusal")?
    else {
        return Err("the first failure must remain typed IncompleteExport".into());
    };
    let health = refusal.health();
    assert!(
        !created.get(),
        "the source refusal cannot create or replace an output"
    );
    assert_eq!(
        std::fs::read(&output)?,
        b"previous complete catalog",
        "the actual previous file survives normal refusal"
    );
    Ok(serde_json::to_value(health)?)
}

fn same_identity(normal: &Value, row: &Value, published: &Value, diagnostic: &Value) -> TestResult {
    for field in ["guid", "recordIndex", "recordPath", "sourceType"] {
        let first = check::cell(row, field, "present")?;
        let second = check::cell(diagnostic, field, "present")?;
        for property in ["state", "containsEncryptedSource", "sourceSetComplete"] {
            assert_eq!(
                first.get(property),
                second.get(property),
                "{field}/{property} is preserved without comparing document-local IDs"
            );
        }
        if matches!(field, "recordPath" | "sourceType") {
            assert_eq!(
                check::text_value(normal, first)?,
                check::text_value(published, second)?,
                "{field} retains the same exact text in each report's own storage"
            );
        } else {
            assert_eq!(
                first.get("value"),
                second.get("value"),
                "{field}/value is preserved without comparing document-local IDs"
            );
        }
    }
    Ok(())
}
