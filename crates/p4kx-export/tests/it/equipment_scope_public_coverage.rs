// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{self, EquipmentSnapshot};
use p4kx_extraction::{Archive, DataForgeEntry};

use super::scope_fixture::{self as fixture, Fault};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn bounds_scope_reports_an_original_unclassified_component_without_losing_the_prefix() -> TestResult
{
    let (_folder, archive, entry) = prepared(Fault::Component)?;
    bounds(&archive, &entry, &[0])
}

#[test]
fn attachment_scope_reports_an_original_unclassified_component_without_losing_the_prefix()
-> TestResult {
    let (_folder, archive, entry) = prepared(Fault::Component)?;
    attachments(&archive, &entry, &[0])
}

#[test]
fn bounds_scope_reports_an_unreadable_target_in_the_declared_occupancy_family() -> TestResult {
    let (_folder, archive, entry) = prepared(Fault::AttachDef)?;
    bounds(&archive, &entry, &[0])
}

#[test]
fn attachment_scope_observes_the_actual_unreadable_attachment_target() -> TestResult {
    let (_folder, archive, entry) = prepared(Fault::AttachDef)?;
    attachments(&archive, &entry, &[0])
}

#[test]
fn original_null_and_readable_foreign_components_are_healthy_scoped_exclusions() -> TestResult {
    for fault in [Fault::Null, Fault::Foreign] {
        let (_folder, archive, entry) = prepared(fault)?;
        bounds(&archive, &entry, &[])?;
        attachments(&archive, &entry, &[])?;
    }
    Ok(())
}

fn prepared(fault: Fault) -> TestResult<(tempfile::TempDir, Archive, DataForgeEntry)> {
    let (folder, archive) = common::open(&fixture::database(fault), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    fixture::prove_original(&entry, fault)?;
    Ok((folder, archive, entry))
}

fn bounds(archive: &Archive, entry: &DataForgeEntry, expected: &[u32]) -> TestResult {
    equipment::with_item_bounds_from(
        archive,
        entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            coverage(&snapshot, expected);
            if expected.is_empty() {
                assert_eq!(
                    snapshot.rows().len(),
                    2,
                    "null and foreign components emit no additional bounds row"
                );
            }
            prefix(
                snapshot.rows().iter().map(|row| {
                    (
                        row.owner_record_index().value().copied(),
                        row.component_ordinal().value().copied(),
                    )
                }),
                &[0, 1],
            );
        },
    )?;
    Ok(())
}

fn attachments(archive: &Archive, entry: &DataForgeEntry, expected: &[u32]) -> TestResult {
    equipment::with_attachment_constraints_from(
        archive,
        entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            coverage(&snapshot, expected);
            if expected.is_empty() {
                assert_eq!(
                    snapshot.rows().len(),
                    1,
                    "null and foreign components emit no additional attachment row"
                );
            }
            prefix(
                snapshot.rows().iter().map(|row| {
                    (
                        row.owner_record_index().value().copied(),
                        row.component_ordinal().value().copied(),
                    )
                }),
                &[0],
            );
        },
    )?;
    Ok(())
}

fn coverage<Row>(snapshot: &EquipmentSnapshot<'_, Row>, expected: &[u32]) {
    let coverage = snapshot.coverage();
    assert!(
        coverage.unreadable_headers.is_empty(),
        "the original header was successfully read"
    );
    assert!(
        coverage.unreadable_bodies.is_empty(),
        "the original body retained its Value::Error in place"
    );
    assert_eq!(
        coverage.incomplete_fields.as_slice(),
        expected,
        "the scoped query must retain uncertainty at the original owner record"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "the scoped configurations require no localization"
    );
    assert!(
        snapshot.available_languages().is_empty(),
        "no locale payload was read"
    );
}

fn prefix(actual: impl Iterator<Item = (Option<u32>, Option<u32>)>, expected: &[u32]) {
    let positions: Vec<_> = actual.collect();
    for &ordinal in expected {
        assert_eq!(
            positions
                .iter()
                .filter(|&&(owner, slot)| owner == Some(0) && slot == Some(ordinal))
                .count(),
            1,
            "the readable original prefix component {ordinal} survives exactly once"
        );
    }
}
