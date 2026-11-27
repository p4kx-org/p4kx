// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogPublicationMode};
use p4kx_extraction::{CancelToken, DataForgeEntry};

use super::{Layout, attachments, bounds, catalog, common, database, entities, parity};
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::sdk_full_proof as full_proof;

#[test]
fn attachment_visitor_preserves_guid_aliases_and_repeated_physical_cells() -> TestResult {
    let (_folder, archive) =
        common::open(&attachments::original_restrictions(), DcbOrigin::Encrypted)?;
    let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    let entry = DataForgeEntry::read(&archive)?;
    let selection = CatalogRecordSelection::Guid(common::guid(attachments::OWNER).parse()?);
    let aggregate = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .with_attachment_constraints(selection, None, |snapshot| snapshot.coverage().clone())?;
    let mut indices = Vec::new();
    let summary = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .visit_attachment_constraints(selection, None, |index, snapshot| -> TestResult {
            indices.push(index);
            parity::attachment(snapshot, &expected, Some(&[index]))
        })?;
    assert_eq!(
        indices,
        [0, 1],
        "both physical occurrences of the GUID stay distinct"
    );
    assert_eq!(
        (summary.owner_count(), summary.row_count()),
        (2, 3),
        "all physical rows survive"
    );
    assert_eq!(
        summary.coverage(),
        &aggregate,
        "the aggregate coverage remains exact"
    );
    Ok(())
}

#[test]
fn bounds_visitor_keeps_canonical_occupancy_and_coverage_without_locales() -> TestResult {
    let (_folder, archive) = common::open(&bounds::complete(), DcbOrigin::Encrypted)?;
    let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    let session = full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let aggregate = parsed.with_item_bounds(CatalogRecordSelection::All, None, |snapshot| {
        snapshot.coverage().clone()
    })?;
    let mut counts = Vec::new();
    let summary = parsed.visit_item_bounds(
        CatalogRecordSelection::All,
        None,
        |index, snapshot| -> TestResult {
            counts.push((index, snapshot.rows().len()));
            parity::bounds_rows(snapshot, &expected, Some(&[index]))
        },
    )?;
    assert_eq!(
        counts
            .iter()
            .filter(|(_, rows)| *rows != 0)
            .copied()
            .collect::<Vec<_>>(),
        [(0, 9), (1, 1)],
        "the rows retain their physical owners and order"
    );
    assert_eq!(summary.row_count(), 10, "all original components survive");
    assert_eq!(
        summary.owner_count(),
        counts.len(),
        "readable candidates without rows are counted"
    );
    assert_eq!(
        summary.coverage(),
        &aggregate,
        "aggregate read and field coverage"
    );
    Ok(())
}

#[test]
fn attachment_visits_agree_with_normal_and_stop_at_callback_cancellation() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Complete), DcbOrigin::Plain)?;
    let expected = catalog(&archive, CatalogPublicationMode::Normal)?;
    let session = full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let complete = session.visit_attachment_constraints(
        CatalogRecordSelection::All,
        None,
        |index, snapshot| parity::attachment(snapshot, &expected, Some(&[index])),
    )?;
    assert!(
        matches!(complete.availability(), DatasetAvailability::Available),
        "declared family"
    );
    let cancel = CancelToken::new();
    let mut visited = Vec::new();
    let cancelled = session.visit_attachment_constraints(
        CatalogRecordSelection::All,
        Some(&cancel),
        |index, _| -> Result<(), CatalogError> {
            visited.push(index);
            cancel.cancel();
            Ok(())
        },
    );
    assert_eq!(visited, [0], "no callback follows cancellation");
    assert!(
        matches!(cancelled, Err(CatalogError::Cancelled)),
        "a cancelled visit yields no terminal success"
    );
    Ok(())
}

#[test]
fn bound_visitor_keeps_partial_fields_and_rejects_foreign_entries() -> TestResult {
    let (_folder, archive) = common::open(&entities::wrong_dimension_type(), DcbOrigin::Plain)?;
    let expected =
        equipment::with_item_bounds(&archive, CatalogRecordSelection::All, None, |snapshot| {
            snapshot.coverage().clone()
        })?;
    assert!(
        !expected.incomplete_fields.is_empty(),
        "invalid dimensions create a partial original bound"
    );
    let summary = equipment::visit_item_bounds(
        &archive,
        CatalogRecordSelection::All,
        None,
        |index, snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().incomplete_fields.contains(&index),
                expected.incomplete_fields.contains(&index),
                "the callback retains its own incomplete owner"
            );
            Ok(())
        },
    )?;
    assert_eq!(
        summary.coverage(),
        &expected,
        "partial owners reach the terminal coverage"
    );
    let (_foreign, other) = common::open(&bounds::complete(), DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&other)?;
    let result = equipment::visit_item_bounds_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |_, _| -> Result<(), CatalogError> { panic!("a foreign Entry cannot reach the visitor") },
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "strict original Entry binding"
    );
    Ok(())
}
