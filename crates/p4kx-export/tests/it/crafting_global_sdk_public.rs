// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::crafting;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, PhysicalSourceKind, SourceValueState, SourcedValue,
};
use serde_json::Value;

use super::{fixture, walk};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn crafting_globals_getters_match_actual_canonical_rows_in_all_origin_states() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&fixture::database(), origin)?;
        let catalog = canonical(&archive)?;
        let session = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?;
        session.with_crafting_global_params(CatalogRecordSelection::All, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                2,
                "same-GUID physical owners remain separate"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "declared family"
            );
            assert!(
                snapshot.coverage().unreadable_bodies.is_empty(),
                "no dependency bodies opened"
            );
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "all selected original cells read"
            );
            assert!(
                snapshot
                    .source_inventory()
                    .iter_sources()
                    .all(|source| match source.kind() {
                        PhysicalSourceKind::ArchiveEntry(entry) =>
                            entry.path().value().is_some_and(|path| path
                                .replace('\\', "/")
                                .eq_ignore_ascii_case("Data/Game2.dcb")),
                        PhysicalSourceKind::ArchiveDirectory(_) => true,
                        _ => false,
                    }),
                "no locale or foreign payload enters this source inventory"
            );
            for row in snapshot.rows() {
                assert_eq!(
                    row.source().guid().origin().contains_encrypted_source(),
                    marker,
                    "header uses its original DCB"
                );
                assert_eq!(
                    row.dismantle_blacklist_resources()
                        .origin()
                        .contains_encrypted_source(),
                    marker,
                    "each original array has its own marker"
                );
            }
            walk::snapshot(&snapshot, &catalog)
        })??;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn crafting_global_entry_session_and_parsed_selection_keep_original_slots() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = canonical(&archive)?;
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    session.with_crafting_global_params(
        CatalogRecordSelection::RecordIndex(1),
        None,
        |snapshot| walk::snapshot(&snapshot, &catalog),
    )??;
    let parsed = session.parsed(None)?;
    parsed.with_crafting_global_params(
        CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
        None,
        |snapshot| {
            let [primary, alias] = snapshot.rows() else {
                return Err("two physical aliases required".into());
            };
            let defaults = primary
                .default_blueprint_selection()
                .value()
                .ok_or("original selection")?;
            let slots = defaults
                .blueprint_records()
                .value()
                .ok_or("original defaults array")?;
            assert_eq!(slots.len(), 4, "repeat, null and unresolved slots survive");
            assert_eq!(
                slots.iter().map(SourcedValue::state).collect::<Vec<_>>(),
                [
                    SourceValueState::Present,
                    SourceValueState::Null,
                    SourceValueState::Present,
                    SourceValueState::Present
                ],
                "original slot states"
            );
            assert_eq!(
                alias.source().record_index().value(),
                Some(&1),
                "physical identity"
            );
            walk::snapshot(&snapshot, &catalog)
        },
    )??;
    session.with_crafting_global_params(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| walk::snapshot(&snapshot, &catalog),
    )??;
    Ok(())
}

#[test]
fn crafting_global_cancellation_never_publishes_outer_success() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let result =
        crafting::with_global_params(&archive, CatalogRecordSelection::All, Some(&cancel), |_| {
            panic!("pre-cancelled query cannot invoke consumer")
        });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "pre-cancel propagates"
    );
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let cancel = CancelToken::new();
    let result =
        session.with_crafting_global_params(CatalogRecordSelection::All, Some(&cancel), |_| {
            cancel.cancel();
        });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "post-callback outer cancellation"
    );
    Ok(())
}

#[test]
fn crafting_global_query_rejects_foreign_decoded_entries() -> TestResult {
    let (_left, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let (_right, foreign) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let result = crafting::with_global_params_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |_| panic!("foreign Entry cannot be admitted"),
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "foreign source rejected"
    );
    Ok(())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let mut options = CatalogOptions::new(std::time::UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(archive, options.with_full_provenance())?.json(),
    )?)
}
