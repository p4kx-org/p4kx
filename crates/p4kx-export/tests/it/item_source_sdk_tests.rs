// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_export::catalog_queries::equipment::{self, ItemSource, ItemSourceSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogError, CatalogPublicationMode, ItemClass};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeEntry, Language, PhysicalSourceKind, SourceLocation,
    SourceValueState,
};
use serde_json::Value;

use crate::canonical_header::wire::Wire;
use crate::catalog_resources::resource_consumer_walk;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

use super::fixture::{self, Layout};
use crate::sdk_full_proof as full_proof;

#[test]
fn item_source_sdk_matches_physical_aliases_and_preserves_original_english_scope() -> TestResult {
    let (_folder, archive) = fixture::open(Layout::Aliases, DcbOrigin::Plain, true, false)?;
    let catalog = canonical(&archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[Language::German]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_item_sources(
        CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
        None,
        |snapshot| verify_snapshot(&snapshot, &catalog, &[0, 1]),
    )??;
    parsed.with_item_sources(CatalogRecordSelection::RecordIndex(1), None, |snapshot| {
        verify_snapshot(&snapshot, &catalog, &[1])
    })??;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?.with_item_sources(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| verify_snapshot(&snapshot, &catalog, &[0]),
    )??;
    Ok(())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        p4kx_export::build_catalog(archive, options.with_full_provenance())?.json(),
    )?)
}

fn verify_snapshot(
    snapshot: &ItemSourceSnapshot<'_>,
    catalog: &Value,
    selected: &[u32],
) -> TestResult {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "Item family is declared"
    );
    assert_eq!(
        snapshot.rows().len(),
        selected.len(),
        "physical selection preserves cardinality"
    );
    assert!(
        snapshot.coverage().unreadable_headers.is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty()
            && snapshot.coverage().incomplete_fields.is_empty(),
        "the requested Item source cells are proved complete"
    );
    check_language_scope(snapshot);
    for (row, &index) in snapshot.rows().iter().zip(selected) {
        assert_eq!(
            row.source().record_index().value(),
            Some(&index),
            "physical owners stay distinct"
        );
        let canonical = proof::rows(catalog, "/items")?
            .iter()
            .find(|value| {
                value.pointer("/recordIndex/value").and_then(Value::as_u64)
                    == Some(u64::from(index))
            })
            .ok_or("actual canonical Item source row")?;
        verify_cells(row, Wire::new(canonical, snapshot.source_inventory()))?;
        verify_english_lookup(row, snapshot)?;
    }
    Ok(())
}

fn check_language_scope(snapshot: &ItemSourceSnapshot<'_>) {
    assert!(
        snapshot.requested_languages().is_empty()
            && snapshot.available_languages().is_empty()
            && snapshot.public_names().is_empty()
            && snapshot.game_texts().is_empty(),
        "the projection publishes no presentation locale or translation rows"
    );
    assert!(
        snapshot.source_inventory().iter_sources().all(|source| {
            !matches!(source.kind(), PhysicalSourceKind::ArchiveEntry(entry)
                if entry.path().value().is_some_and(|path| path.eq_ignore_ascii_case(Language::German.global_ini_path())))
        }),
        "the session's unrelated locale is outside the projection's observed sources"
    );
}

fn verify_cells(row: &ItemSource<'_>, wire: Wire<'_>) -> TestResult {
    resource_consumer_walk::visit_header(row.source(), wire)?;
    let selection = wire.field("selection")?;
    selection
        .field("attachmentComponentOrdinal")?
        .leaf(row.selection().attachment_component_ordinal())?;
    selection
        .field("physicsComponentOrdinal")?
        .leaf(row.selection().physics_component_ordinal())?;
    selection
        .field("purchasableComponentOrdinal")?
        .leaf(row.selection().purchasable_component_ordinal())?;
    wire.field("itemClass")?.leaf(row.item_class())?;
    wire.field("playability")?.leaf(row.playability())?;
    let serialized = serde_json::to_value(row)?;
    assert!(
        serialized.get("variantOf").is_none(),
        "a source-only response has no variant decision"
    );
    Ok(())
}

fn verify_english_lookup(row: &ItemSource<'_>, snapshot: &ItemSourceSnapshot<'_>) -> TestResult {
    let expected = if row.source().record_index().value() == Some(&0) {
        ItemClass::Military
    } else {
        ItemClass::Stealth
    };
    assert_eq!(
        row.item_class().value(),
        Some(&expected),
        "the original English Description class"
    );
    assert_eq!(
        row.item_class().origin().contains_encrypted_source(),
        Some(true),
        "the exact English source is encrypted"
    );
    assert_eq!(
        row.playability().origin().contains_encrypted_source(),
        Some(false),
        "playability uses the original plain keys"
    );
    let inventory = snapshot.source_inventory();
    let uses = row.item_class().origin().source_uses_refs();
    let index = u64::from(
        *row.source()
            .record_index()
            .value()
            .ok_or("physical owner index")?,
    );
    let lookup = inventory
        .iter_uses()
        .find(|usage| {
            uses.contains(usage.id())
                && matches!(
                    usage.location(),
                    SourceLocation::LocalizationLookup {
                        language: Language::English,
                        ..
                    }
                )
        })
        .ok_or("exact English Description lookup")?;
    let SourceLocation::LocalizationLookup { selector, .. } = lookup.location() else {
        return Err("lookup location expected".into());
    };
    let original = inventory
        .iter_uses()
        .find(|usage| usage.id() == selector)
        .ok_or("observed Locale selector")?;
    assert!(
        matches!(original.location(), SourceLocation::Subresource { namespace, indices, .. }
            if namespace == "dataforgeValue" && indices.first() == Some(&index)),
        "the lookup selector resolves to the same original Record's authored field"
    );
    Ok(())
}

#[test]
fn item_source_sdk_keeps_first_null_and_incomplete_prefix_distinct() -> TestResult {
    for (layout, incomplete, state) in [
        (Layout::NullFirst, false, SourceValueState::Absent),
        (Layout::UnknownPrefix, true, SourceValueState::Present),
    ] {
        let (_folder, archive) = fixture::open(layout, DcbOrigin::Plain, false, false)?;
        equipment::with_item_sources(
            &archive,
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| {
                let [row] = snapshot.rows() else {
                    return Err("one selected original row".into());
                };
                assert_eq!(
                    row.item_class().state(),
                    state,
                    "the first original position controls classification"
                );
                assert_eq!(
                    row.item_class().origin().source_set_complete(),
                    !incomplete,
                    "an unknown earlier occurrence leaves the scope incomplete"
                );
                assert_eq!(
                    snapshot.coverage().incomplete_fields.len(),
                    usize::from(incomplete),
                    "query coverage follows the field evidence"
                );
                if incomplete {
                    assert_eq!(
                        snapshot.coverage().incomplete_fields,
                        [0],
                        "only the physical owner is marked incomplete"
                    );
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(())
}

#[test]
fn empty_item_source_selection_consults_no_locale_payloads() -> TestResult {
    let (_folder, archive) = fixture::open(Layout::Normal, DcbOrigin::Plain, true, true)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    session.with_item_sources(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "the selected physical position is absent"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "the family remains available"
            );
            check_language_scope(&snapshot);
            assert!(
                snapshot.source_inventory().iter_sources().all(|source| {
                    !matches!(source.kind(), PhysicalSourceKind::ArchiveEntry(entry)
                    if entry.path().value().is_some_and(|path| path.eq_ignore_ascii_case(Language::English.global_ini_path())))
                }),
                "an empty owner selection requires no semantic English payload"
            );
        },
    )?;
    Ok(())
}

#[test]
fn item_source_sdk_refuses_an_entry_from_an_equal_foreign_archive() -> TestResult {
    let (_folder, archive) = fixture::open(Layout::Normal, DcbOrigin::Plain, false, false)?;
    let (_foreign_folder, foreign) = fixture::open(Layout::Normal, DcbOrigin::Plain, false, false)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let result: Result<(), CatalogError> = equipment::with_item_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |_| panic!("a foreign original cannot supply a result"),
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "physical provenance is checked before projection"
    );
    Ok(())
}

#[test]
fn item_source_sdk_preserves_outer_cancellation_before_and_after_the_callback() -> TestResult {
    let (_folder, archive) = fixture::open(Layout::Normal, DcbOrigin::Plain, false, false)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::RecordIndex(0);
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let pre: Result<(), CatalogError> =
        parsed.with_item_sources(selection, Some(&cancelled), |_| {
            panic!("a pre-cancelled projection cannot supply a result");
        });
    assert!(
        matches!(pre, Err(CatalogError::Cancelled)),
        "pre-cancellation is an outer error"
    );
    let cancel = CancelToken::new();
    let called = Cell::new(false);
    let post = parsed.with_item_sources(selection, Some(&cancel), |_| {
        called.set(true);
        cancel.cancel();
    });
    assert!(
        called.get() && matches!(post, Err(CatalogError::Cancelled)),
        "after-callback cancellation prevents outer success"
    );
    Ok(())
}
