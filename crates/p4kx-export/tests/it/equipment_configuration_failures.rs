// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogError, CatalogPublicationMode};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};

use super::{Layout, attachments, bounds, catalog, common, database, parity};
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::sdk_full_proof as full_proof;

#[test]
fn attachment_query_preserves_selected_failures_and_excludes_unrelated_port_errors() -> TestResult {
    for (layout, incomplete) in [
        (Layout::UnrelatedInvalid, true),
        (Layout::Unreadable, false),
        (Layout::Absent, false),
    ] {
        let (_folder, archive) = common::open(&database(layout), DcbOrigin::Encrypted)?;
        let expected = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_attachment_constraints(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| {
                parity::attachment(&snapshot, &expected, Some(&[0]))?;
                assert_eq!(
                    !snapshot.coverage().incomplete_fields.is_empty(),
                    incomplete,
                    "only errors in attachment fields affect this request"
                );
                Ok::<_, Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(())
}

#[test]
fn configuration_queries_do_not_open_unreadable_locale_bodies() -> TestResult {
    let (_folder, archive) = without_readable_locales(&attachments::original_restrictions())?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    session.with_attachment_constraints(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            3,
            "all actual attachment components are still readable"
        );
        assert!(
            snapshot.requested_languages().is_empty() && snapshot.available_languages().is_empty(),
            "unused session languages never trigger a locale read"
        );
    })?;
    parsed.with_attachment_constraints(CatalogRecordSelection::All, None, |snapshot| {
        assert!(
            snapshot.public_names().is_empty() && snapshot.game_texts().is_empty(),
            "no text definitions are fabricated"
        );
    })?;
    let (_folder, archive) = without_readable_locales(&bounds::complete())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    session.with_item_bounds(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(snapshot.rows().len(), 10, "bounds need no locale payloads");
        assert!(
            snapshot.localization_notes().is_empty(),
            "unopened locale bytes have no decode notes"
        );
    })?;
    Ok(())
}

#[test]
fn cancelled_configuration_queries_never_invoke_the_consumer() -> TestResult {
    let (_folder, archive) = common::open(&bounds::complete(), DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let bounds: Result<(), CatalogError> =
        equipment::with_item_bounds(&archive, CatalogRecordSelection::All, Some(&cancel), |_| {
            panic!("a cancelled bounds query has no output")
        });
    assert!(
        matches!(bounds, Err(CatalogError::Cancelled)),
        "bounds cancellation precedes callbacks"
    );
    let attachments: Result<(), CatalogError> = equipment::with_attachment_constraints(
        &archive,
        CatalogRecordSelection::All,
        Some(&cancel),
        |_| panic!("a cancelled attachment query has no output"),
    );
    assert!(
        matches!(attachments, Err(CatalogError::Cancelled)),
        "attachment cancellation precedes callbacks"
    );
    Ok(())
}

#[test]
fn configuration_from_entry_rejects_a_foreign_original_input() -> TestResult {
    let database = attachments::original_restrictions();
    let (_first, archive) = common::open(&database, DcbOrigin::Plain)?;
    let (_second, foreign) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let result: Result<(), CatalogError> = equipment::with_attachment_constraints_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |_| panic!("a foreign attachment input has no output"),
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "attachment original Entry binding is checked"
    );
    let result: Result<(), CatalogError> = equipment::with_item_bounds_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |_| panic!("a foreign bounds input has no output"),
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "bounds original Entry binding is checked"
    );
    Ok(())
}

fn without_readable_locales(
    database: &DataForgeBuilder,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("unlocalized.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .entry(
            TestEntry::zstd(Language::English.global_ini_path(), b"not read")
                .with_payload(b"invalid zstd frame"),
        )
        .entry(
            TestEntry::zstd(Language::German.global_ini_path(), b"not read")
                .with_payload(b"invalid zstd frame"),
        )
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}
