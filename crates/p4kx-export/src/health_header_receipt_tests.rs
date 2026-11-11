// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    InputScope, Record, SourceInventory, SourceLocation, SourceUseDescription, SourceUseRole,
    SourceValue,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::HeaderReadReceipt;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::RecordSubject;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner").field("value", TestType::UInt32))
        .record(
            TestRecord::new(0x7111, "Owner", "Owner.One", "owner.xml")
                .value("value", TestValue::UInt32(11)),
        )
        .build()
}

#[test]
fn equal_original_headers_do_not_admit_a_foreign_receipt_merge() -> TestResult {
    let bytes = bytes();
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("first original Record")?;
        let receipt = HeaderReadReceipt::read(input, &record)?;
        let subject = RecordSubject::capture(input, &record)?;
        test_support::with_input(&bytes, |other_forge, other_input| {
            let other = other_forge.record(0).ok_or("second original Record")?;
            equal_headers(&record, &other)?;
            let foreign = HeaderReadReceipt::read(other_input, &other)?;
            let other_subject = RecordSubject::capture(other_input, &other)?;
            assert!(
                !input
                    .origins()
                    .uses
                    .is_same_inventory(&other_input.origins().uses),
                "two actual preparations retain distinct original inventories",
            );
            assert!(
                matches!(
                    receipt.merge(&foreign),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "header value equality cannot admit foreign original frames",
            );
            assert!(
                matches!(
                    subject.clone().merge(other_subject),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "the whole subject enforces the same original-inventory boundary",
            );
            rejected_finish(&receipt, &other_input.origins().uses)?;
            Ok(())
        })?;
        let clone = receipt.clone();
        clone.merge(&receipt)?;
        let retained = clone.record(&input.origins().uses)?;
        assert_eq!(
            retained
                .record_index
                .into_cell(&input.origins().uses)
                .value(),
            Some(&record.index()),
            "an unchanged original clone remains usable"
        );
        Ok(())
    })
}

fn equal_headers(first: &Record<'_>, second: &Record<'_>) -> TestResult {
    assert_eq!(first.index(), second.index(), "original indices coincide");
    assert_eq!(first.guid(), second.guid(), "original GUIDs coincide");
    assert_eq!(
        first.record_path()?,
        second.record_path()?,
        "original paths coincide"
    );
    assert_eq!(
        first.type_name()?,
        second.type_name()?,
        "original types coincide"
    );
    Ok(())
}

fn rejected_finish(
    receipt: &HeaderReadReceipt,
    sources: &p4kx_extraction::SourceUses,
) -> TestResult {
    let before = sources.freeze()?;
    assert!(
        matches!(
            receipt.record(sources),
            Err(CatalogError::Accounting(
                ExportAccountingError::UnboundHealthSubject
            ))
        ),
        "a foreign report cannot retain the original header"
    );
    assert!(
        matches!(
            receipt.subject(sources),
            Err(CatalogError::Accounting(
                ExportAccountingError::UnboundHealthSubject
            ))
        ),
        "a foreign report cannot retain the original diagnostic subject"
    );
    assert_eq!(
        before,
        sources.freeze()?,
        "rejection registers no foreign output uses"
    );
    Ok(())
}

#[test]
fn unretained_header_reads_register_no_guid_or_name_payload() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let baseline = input.origins().uses.freeze()?;
        metadata_payload_baseline(&baseline);
        let receipt = HeaderReadReceipt::read(input, &record)?;
        assert!(
            matches!(receipt.path, SourceValue::Present(_)),
            "the actual readable path was captured before deciding retention"
        );
        let after_read = input.origins().uses.freeze()?;
        assert_eq!(
            payloads(&after_read),
            payloads(&baseline),
            "the unretained header adds no payload claim beyond the actual directory baseline",
        );
        let retained = receipt.subject(&input.origins().uses)?;
        assert_eq!(
            retained.guid.into_cell(&input.origins().uses).value(),
            Some(&record.guid()),
            "retention keeps the actual original GUID"
        );
        one_retained_guid_payload(&baseline, &input.origins().uses.freeze()?, record.index())
    })
}

fn payloads(inventory: &SourceInventory) -> Vec<&SourceUseDescription> {
    inventory
        .uses()
        .iter()
        .filter(|usage| usage.role() == SourceUseRole::Payload)
        .collect()
}

fn metadata_payload_baseline(inventory: &SourceInventory) {
    let baseline = payloads(inventory);
    assert!(
        !baseline.is_empty(),
        "the actual Archive admission has already retained its directory metadata facts",
    );
    assert!(
        baseline.iter().all(
            |usage| usage.scope() == InputScope::ArchiveDirectoryMetadata
                && matches!(usage.location(), SourceLocation::Metadata { .. })
        ),
        "the existing payload uses are directory metadata, not original Record header payloads",
    );
}

fn one_retained_guid_payload(
    baseline: &SourceInventory,
    retained: &SourceInventory,
    record_index: u32,
) -> TestResult {
    let payloads = payloads(retained);
    let added: Vec<_> = payloads
        .iter()
        .copied()
        .filter(|usage| !baseline.uses().iter().any(|prior| prior.id() == usage.id()))
        .collect();
    assert_eq!(
        added.len(),
        1,
        "only the retained GUID contributes a new header payload",
    );
    let guid = added.first().ok_or("the one new original GUID payload")?;
    assert_eq!(
        guid.location(),
        &RecordHeaderField::Guid.location(record_index),
        "the sole retained payload is the exact original GUID header coordinate",
    );
    assert_eq!(
        guid.scope(),
        InputScope::Payload,
        "the actual DCB header byte domain"
    );
    assert_eq!(
        payloads.len(),
        self::payloads(baseline).len() + 1,
        "every pre-existing metadata payload remains and only one header payload is added",
    );
    Ok(())
}
#[test]
fn different_original_inputs_in_one_inventory_do_not_merge_physical_records() -> TestResult {
    test_support::with_shared_inventory_inputs(
        &bytes(),
        |forge, input, other_forge, other_input| {
            let record = forge.record(0).ok_or("first admitted original")?;
            let other = other_forge.record(0).ok_or("second admitted original")?;
            equal_headers(&record, &other)?;
            assert!(
                input
                    .origins()
                    .uses
                    .is_same_inventory(&other_input.origins().uses),
                "the two actual source inputs share the existing report inventory"
            );
            let first = HeaderReadReceipt::read(input, &record)?;
            let foreign = HeaderReadReceipt::read(other_input, &other)?;
            assert!(
                matches!(
                    foreign.check_input(input),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "one report inventory does not make a foreign physical input the owning catalog input"
            );
            let foreign_subject = RecordSubject::capture(other_input, &other)?;
            assert!(
                matches!(
                    foreign_subject.check_input(input),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "the actual finish guard rejects the foreign original subject before materialization"
            );
            first.check_input(input)?;
            assert_ne!(
                first.frame.index.description().source(),
                foreign.frame.index.description().source(),
                "real EntryOrigin admissions identify different physical inputs"
            );
            assert!(
                matches!(
                    first.merge(&foreign),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "a shared registry does not authorize a different original physical Record"
            );
            let repeated = HeaderReadReceipt::read(input, &record)?;
            let same = first;
            same.merge(&repeated)?;
            assert!(
                std::ptr::eq(
                    same.frame.index.description(),
                    repeated.frame.index.description()
                ),
                "repeated admission preserves the same actual registered owner selector"
            );
            Ok(())
        },
    )
}
