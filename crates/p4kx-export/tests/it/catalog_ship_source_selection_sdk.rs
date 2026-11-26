// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::cell::Cell;

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::equipment::{self, ShipSourceSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{CancelToken, DataForgeEntry};
use serde_json::Value;

use super::fixture::{self, Case};
use crate::catalog_ship_base::source_sdk_walk;
use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;

#[test]
fn direct_ship_sdk_keeps_guid_aliases_and_exact_physical_selection() -> TestResult {
    let (_folder, archive) = fixture::open(Case::Aliases)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let guid = forge.record(0).ok_or("actual first physical Ship")?.guid();
    let catalog = fixture::catalog(&archive)?;
    let languages = CatalogLanguageSelection::Selected(&[]);
    full_proof::open(&archive, languages, None)?.with_ship_sources(
        CatalogRecordSelection::All,
        None,
        |snapshot| aliases(&snapshot, &catalog, &[0, 1], &[10.0, 20.0]),
    )??;
    let session = full_proof::from_entry(&archive, &entry, languages, None)?;
    session.with_ship_sources(CatalogRecordSelection::Guid(guid), None, |snapshot| {
        aliases(&snapshot, &catalog, &[0, 1], &[10.0, 20.0])
    })??;
    let parsed = session.parsed(None)?;
    parsed.with_ship_sources(CatalogRecordSelection::RecordIndex(1), None, |snapshot| {
        aliases(&snapshot, &catalog, &[1], &[20.0])
    })??;
    parsed.with_ship_sources(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "an absent physical owner has no replacement"
            );
            assert!(
                snapshot.source_inventory().iter_uses().next().is_none(),
                "an empty selection leaves no orphan uses"
            );
            assert!(
                snapshot.dependency_coverage().is_complete(),
                "no selected owner means no failed dependency read"
            );
        },
    )?;
    Ok(())
}

#[test]
fn all_ship_sources_keep_physical_ties_and_failures_across_groups() -> TestResult {
    let (_folder, archive) = fixture::open(Case::GroupedOwners)?;
    let catalog = fixture::catalog(&archive)?;
    let languages = CatalogLanguageSelection::Selected(&[]);
    let indices: Vec<_> = (2..=65).chain([1]).collect();
    let masses: Vec<_> = indices.iter().copied().map(f64::from).collect();
    full_proof::open(&archive, languages, None)?.with_ship_sources(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.coverage().unreadable_bodies,
                [0, 66],
                "unreadable coverage keeps physical order even when body reads follow GUID order"
            );
            aliases(&snapshot, &catalog, &indices, &masses)
        },
    )??;
    Ok(())
}

fn aliases(
    snapshot: &ShipSourceSnapshot<'_>,
    catalog: &Value,
    indices: &[u32],
    masses: &[f64],
) -> TestResult {
    assert_eq!(
        snapshot.rows().len(),
        indices.len(),
        "physical aliases remain distinct"
    );
    for ((row, &index), &mass) in snapshot.rows().iter().zip(indices).zip(masses) {
        assert_eq!(
            row.source().record_index().value(),
            Some(&index),
            "stable physical selection"
        );
        assert_eq!(
            row.mass().value(),
            Some(&mass),
            "the exact physical Physics operand survives shared XML cache"
        );
        let canonical = proof::rows(catalog, "/ships")?
            .iter()
            .find(|row| {
                row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
            })
            .ok_or("canonical physical Ship")?;
        source_sdk_walk::row(row, snapshot.source_inventory(), canonical)?;
        assert_eq!(
            row.mass().origin().contains_encrypted_source(),
            Some(true),
            "XML membership remains an encrypted contributor"
        );
    }
    assert!(
        snapshot.dependency_coverage().is_complete(),
        "healthy cached callers retain complete dependency proof"
    );
    Ok(())
}

#[test]
fn direct_ship_sdk_cancellation_has_no_published_consumer_result() -> TestResult {
    let (_folder, archive) = fixture::open(Case::Parts)?;
    let cancel = CancelToken::new();
    let called = Cell::new(false);
    cancel.cancel();
    let result = equipment::with_ship_sources(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        Some(&cancel),
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "early cancellation stays an outer error"
    );
    assert!(
        !called.get(),
        "early cancellation precedes consumer callback"
    );
    let cancel = CancelToken::new();
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let result = parsed.with_ship_sources(CatalogRecordSelection::All, Some(&cancel), |_| {
        called.set(true);
        cancel.cancel();
    });
    assert!(
        called.get(),
        "the second cancellation follows a genuine completed snapshot callback"
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "the session rechecks cancellation after the callback"
    );
    parsed.with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            1,
            "a canceled request does not poison session reuse"
        );
    })?;
    Ok(())
}

#[test]
fn direct_ship_sdk_rejects_a_foreign_dataforge_entry_before_callback() -> TestResult {
    let (_folder, archive) = fixture::open(Case::Parts)?;
    let (_foreign_folder, foreign) = fixture::open(Case::Parts)?;
    let entry = DataForgeEntry::read(&foreign)?;
    let called = Cell::new(false);
    let result = equipment::with_ship_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(CatalogError::SourceOriginMismatch)),
        "same bytes in another Archive cannot authorize a source"
    );
    assert!(
        !called.get(),
        "foreign input cannot expose a consumer snapshot"
    );
    Ok(())
}
