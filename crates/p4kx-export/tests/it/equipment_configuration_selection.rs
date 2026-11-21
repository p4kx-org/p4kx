// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::DataForgeEntry;

use super::{attachments, bounds, catalog, common, parity};
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

#[test]
fn attachment_queries_keep_guid_aliases_and_repeated_physical_components() -> TestResult {
    let (_folder, archive) =
        common::open(&attachments::original_restrictions(), DcbOrigin::Encrypted)?;
    let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    for (selection, physical, count) in [
        (
            CatalogRecordSelection::Guid(common::guid(attachments::OWNER).parse()?),
            vec![0, 1],
            3,
        ),
        (CatalogRecordSelection::RecordIndex(0), vec![0], 2),
        (CatalogRecordSelection::RecordIndex(1), vec![1], 1),
        (CatalogRecordSelection::RecordIndex(2), vec![2], 0),
    ] {
        equipment::with_attachment_constraints(&archive, selection, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                count,
                "physical attachment occurrences are not merged"
            );
            parity::attachment(&snapshot, &expected, Some(&physical))
        })??;
    }
    Ok(())
}

#[test]
fn bounds_queries_keep_all_units_and_physical_aliases_without_locale_selection() -> TestResult {
    let (_folder, archive) = common::open(&bounds::complete(), DcbOrigin::Encrypted)?;
    let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    for (selection, physical, count) in [
        (
            CatalogRecordSelection::Guid(common::guid(bounds::OWNER).parse()?),
            vec![0, 1],
            10,
        ),
        (CatalogRecordSelection::RecordIndex(0), vec![0], 9),
        (CatalogRecordSelection::RecordIndex(1), vec![1], 1),
        (CatalogRecordSelection::RecordIndex(2), vec![2], 0),
    ] {
        parsed.with_item_bounds(selection, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                count,
                "every original bounds component remains distinct"
            );
            parity::bounds_rows(&snapshot, &expected, Some(&physical))
        })??;
    }
    equipment::with_item_bounds_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| parity::bounds_rows(&snapshot, &expected, Some(&[0])),
    )??;
    Ok(())
}
