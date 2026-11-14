// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogRefusal, build_catalog_from};

use crate::health_origin_fixture::{self as fixture, Input, TestResult};

#[test]
fn actual_refusals_with_equal_storage_indices_keep_their_own_text_and_inventory() -> TestResult {
    let plain = refused("synthetic/refusal-a.xml", Input::Plain)?;
    let encrypted = refused("synthetic/refusal-b.xml", Input::Encrypted)?;
    let plain = report(&plain)?;
    let encrypted_report = report(&encrypted)?;
    let plain_failure = plain.health().failures().first().ok_or("plain failure")?;
    let encrypted_failure = encrypted_report
        .health()
        .failures()
        .first()
        .ok_or("encrypted failure")?;
    assert_eq!(
        plain_failure.details().record_path().value(),
        encrypted_failure.details().record_path().value(),
        "independent real refusals deliberately collide at a report-local atom index",
    );
    own(plain, "synthetic/refusal-a", Some(false))?;
    own(encrypted_report, "synthetic/refusal-b", Some(true))?;
    assert!(
        std::ptr::eq(
            encrypted.source_inventory().ok_or("error inventory")?,
            encrypted_report.source_inventory(),
        ),
        "the error and its inseparable report expose one existing snapshot",
    );
    let moved = encrypted;
    own(report(&moved)?, "synthetic/refusal-b", Some(true))
}

fn refused(file: &str, input: Input) -> TestResult<CatalogError> {
    let source = super::distinct_source(file, input)?;
    build_catalog_from(&source.archive, &source.entry, &fixture::options())
        .err()
        .ok_or_else(|| "the original failing body must refuse normal publication".into())
}

fn report(error: &CatalogError) -> TestResult<&CatalogRefusal> {
    match error {
        CatalogError::IncompleteExport(report) => Ok(report),
        other => Err(format!("unexpected original failure: {other}").into()),
    }
}

fn own(report: &CatalogRefusal, expected: &str, marker: Option<bool>) -> TestResult {
    let failure = report
        .health()
        .failures()
        .first()
        .ok_or("retained failure")?;
    let details = failure.details();
    assert_eq!(failure.record_path(), expected, "own original human path");
    assert_eq!(
        details.record_path_text().value(),
        Some(expected),
        "the private paired view borrows the same semantic occurrence",
    );
    assert_eq!(
        details.record_path().origin().contains_encrypted_source(),
        marker,
        "an unrelated report cannot replace this cell's original contributions",
    );
    super::closed(
        report.source_inventory(),
        details.record_path().origin().source_uses_refs(),
    )?;
    super::closed(
        report.source_inventory(),
        details.guid().origin().source_uses_refs(),
    )
}
