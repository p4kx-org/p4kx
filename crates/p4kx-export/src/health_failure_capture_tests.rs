// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, Record, SourceEvidenceError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::super::catalog_failure_details::CapturedFailure;
use super::finish_failures;
use crate::build::original_input::test_support::{self, TestResult};
use crate::{CatalogError, CatalogReadFailure};

fn bytes(records: u32, path: &str) -> Vec<u8> {
    let mut source = DataForgeBuilder::new().version(8).structure(
        TestStruct::new("Owner")
            .field("value", TestType::UInt32)
            .declared_size(5),
    );
    for index in 0..records {
        source = source.record(
            TestRecord::new(
                u128::from(index) + 1,
                "Owner",
                format!("Owner.{index}"),
                path,
            )
            .value("value", TestValue::UInt32(index)),
        );
    }
    source.build()
}

fn actual_failure<'source>(record: &Record<'source>) -> CatalogReadFailure<'source> {
    let error = record
        .tree()
        .expect_err("the original body has a real layout error");
    assert!(
        matches!(
            error,
            DataForgeError::StructLayout {
                struct_index: 0,
                declared: 5,
                fields: 4
            }
        ),
        "the actual decoder, not a manufactured failure DTO, supplies the error",
    );
    CatalogReadFailure::of(record, &error)
}

#[test]
fn captured_failure_headers_reject_equal_descriptors_from_a_foreign_original() -> TestResult {
    let bytes = bytes(1, "owner.xml");
    let foreign = DataForge::open(&bytes)?;
    let other = foreign.record(0).ok_or("foreign original Record")?;
    let foreign_failure = actual_failure(&other);
    test_support::with_input(&bytes, |forge, input| {
        let selected = forge.record(0).ok_or("admitted original Record")?;
        assert_eq!(
            selected.guid(),
            other.guid(),
            "the actual GUID values coincide"
        );
        assert_eq!(
            selected.index(),
            other.index(),
            "the actual physical indices coincide"
        );
        assert_eq!(
            selected.record_path()?,
            other.record_path()?,
            "the actual paths coincide"
        );
        assert!(!other.is_from(forge), "the original owners are distinct");
        let before = input.origins().uses.freeze()?;
        let rejected = CapturedFailure::read(input, &foreign_failure);
        assert!(
            matches!(
                rejected,
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "equal descriptors cannot authorize foreign header contributions",
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "foreign rejection mints no use"
        );
        own_headers(input, &selected)?;
        Ok(())
    })
}

fn own_headers(
    input: &crate::build::original_input::OriginalCatalogInput<'_, '_>,
    selected: &Record<'_>,
) -> TestResult {
    let own = actual_failure(selected);
    let finished = finish_failures(input, &[own], None)?;
    let row = finished
        .failures
        .first()
        .ok_or("retained original failure")?;
    assert_eq!(
        row.record_index().value(),
        Some(&selected.index()),
        "actual physical position"
    );
    assert_eq!(
        row.guid().value().map(crate::HealthText::as_str),
        Some(selected.guid().to_string().as_str()),
        "actual GUID"
    );
    let path = row.record_path_text();
    assert_eq!(
        path.value(),
        Some(selected.record_path()?),
        "the actual filename conversion"
    );
    assert_eq!(
        path.origin().contains_encrypted_source(),
        Some(false),
        "complete plain header"
    );
    Ok(())
}

#[test]
fn repeated_original_header_text_is_retained_once_and_clone_shares_storage() -> TestResult {
    let path = format!("synthetic/{}.xml", "x".repeat(16_385));
    let bytes = bytes(64, &path);
    test_support::with_input(&bytes, |forge, input| {
        let first = forge.record(0).ok_or("actual first original")?;
        let original_path = first.record_path()?;
        let mut failures = Vec::new();
        for record in forge.records() {
            assert_eq!(
                record.record_path()?,
                original_path,
                "the actual paths are equal"
            );
            assert!(
                std::ptr::eq(record.record_path()?.as_ptr(), original_path.as_ptr()),
                "the original string table shares these exact bytes",
            );
            failures.push(actual_failure(&record));
        }
        let finished = finish_failures(input, &failures, None)?;
        assert_eq!(
            finished.failures.len(),
            64,
            "all distinct physical failures are retained"
        );
        shared_storage(&finished, original_path)
    })
}

fn shared_storage(finished: &super::FinishedReport, path: &str) -> TestResult {
    let mut matching = finished
        .metadata
        .texts()
        .iter()
        .filter_map(|cell| cell.value())
        .filter(|text| text.as_str() == path);
    let stored = matching
        .next()
        .ok_or("the actual retained original path")?
        .as_str();
    assert_eq!(
        1 + matching.count(),
        1,
        "one retained shared text stores the long path"
    );
    assert_eq!(
        finished.metadata.texts().len(),
        3,
        "path, type and actual error text are distinct"
    );
    let first = finished
        .failures
        .first()
        .ok_or("first physical occurrence")?;
    let last = finished.failures.last().ok_or("last physical occurrence")?;
    assert_eq!(
        first.record_path().value(),
        last.record_path().value(),
        "storage is shared"
    );
    assert_ne!(
        first.record_path().origin().source_uses_refs(),
        last.record_path().origin().source_uses_refs(),
        "distinct actual Record positions retain their own header contributions",
    );
    for row in &finished.failures {
        let text = row
            .record_path_text()
            .value()
            .ok_or("the actual present path occurrence")?;
        assert!(
            std::ptr::eq(stored.as_ptr(), text.as_ptr()),
            "each physical occurrence borrows the one stored path allocation",
        );
    }
    clone_and_log_storage(finished)
}

fn clone_and_log_storage(finished: &super::FinishedReport) -> TestResult {
    use std::io::Write as _;

    let cloned = finished.metadata.clone();
    assert!(
        std::ptr::eq(finished.metadata.texts().as_ptr(), cloned.texts().as_ptr()),
        "one immutable owner"
    );
    assert_eq!(
        finished.metadata, cloned,
        "cloning preserves visible metadata values and origins"
    );
    let bytes: usize = finished
        .metadata
        .texts()
        .iter()
        .filter_map(|cell| cell.value())
        .map(|text| text.as_str().len())
        .sum();
    writeln!(
        std::io::stderr().lock(),
        "HEALTH_TEXT_POOL physicalRecords={} texts={} textBytes={bytes}",
        finished.failures.len(),
        finished.metadata.texts().len()
    )?;
    Ok(())
}
#[test]
fn cancelled_failure_capture_stops_before_registering_new_header_contributions() -> TestResult {
    test_support::with_input(&bytes(1, "owner.xml"), |forge, input| {
        let record = forge.record(0).ok_or("actual original failed Record")?;
        let failure = actual_failure(&record);
        let before = input.origins().uses.freeze()?;
        let cancel = p4kx_extraction::CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                finish_failures(input, &[failure], Some(&cancel)),
                Err(CatalogError::Cancelled)
            ),
            "the actual newly introduced capture boundary preserves fatal cancellation",
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "cancellation before capture does not invent header observations",
        );
        Ok(())
    })
}
