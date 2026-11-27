// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::shops::{
    visit_shops, visit_shops_from, visit_shops_with_provenance_mode,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability, ProvenanceMode,
};
use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, StarmapTraversalBudget, build_catalog,
};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::TestResult;
use crate::health_origin_assertions as health;
use crate::shops_fixture::REGISTRY;

#[test]
fn complete_shop_visit_preserves_canonical_order_and_proof_modes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = crate::shop_items_fixture::write_archive(directory.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let budget = StarmapTraversalBudget::default();
    for (mode, expected_path) in [
        (ProvenanceMode::MarkersOnly, "/catalog/shops"),
        (ProvenanceMode::Full, "/shops"),
    ] {
        let mut rows = Vec::new();
        let all = visit_shops_with_provenance_mode::<CatalogError>(
            &archive,
            budget,
            mode,
            None,
            |row| {
                rows.push(serde_json::to_value(row).map_err(CatalogError::Json)?);
                Ok(())
            },
        )?;
        let mut options = CatalogOptions::new(UNIX_EPOCH);
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog(&archive, options.with_provenance_mode(mode))?;
        let normal: Value = serde_json::from_slice(built.json())?;
        let expected = normal
            .pointer(expected_path)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing expected Shop array at {expected_path}"))?;
        assert!(matches!(all.availability, DatasetAvailability::Available));
        assert_eq!(all.row_count, rows.len(), "one callback per admitted Shop");
        assert!(rows.len() > 1, "the fixture exercises canonical key order");
        assert_eq!(&rows, expected, "complete Shop array parity");
        assert!(
            !all.coverage.registry_complete,
            "malformed registry evidence is retained"
        );
        assert!(
            all.coverage.names.is_some(),
            "located rows share one name survey"
        );
        if matches!(mode, ProvenanceMode::Full) {
            health::closure(&json!({"provenance": all.source_inventory, "shops": rows}))?;
        }
    }
    Ok(())
}

#[test]
fn complete_shop_visit_supports_reusable_views_and_stops_on_cancellation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = crate::shop_items_fixture::write_archive(directory.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let budget = StarmapTraversalBudget::default();
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let from = visit_shops_from::<CatalogError>(&archive, &entry, budget, None, |_| Ok(()))?;
    let reused = parsed.visit_shops::<CatalogError>(budget, None, |_| Ok(()))?;
    assert_eq!(
        from.row_count, reused.row_count,
        "decoded and parsed visits match"
    );
    assert_eq!(
        from.coverage, reused.coverage,
        "reusable views observe the same inputs"
    );

    let cancel = CancelToken::new();
    let mut calls = 0;
    let cancelled = parsed.visit_shops::<CatalogError>(budget, Some(&cancel), |_| {
        calls += 1;
        cancel.cancel();
        Ok(())
    });
    assert!(matches!(cancelled, Err(CatalogError::Cancelled)));
    assert_eq!(calls, 1, "no rows are emitted after the consumer cancels");
    let already_cancelled = visit_shops::<CatalogError>(&archive, budget, Some(&cancel), |_| {
        panic!("a cancelled visit must never start a callback")
    });
    assert!(matches!(already_cancelled, Err(CatalogError::Cancelled)));
    Ok(())
}

#[derive(Debug)]
enum ConsumerError {
    Query(CatalogError),
    Stop(usize),
}

impl From<CatalogError> for ConsumerError {
    fn from(error: CatalogError) -> Self {
        Self::Query(error)
    }
}

#[test]
fn complete_shop_visit_returns_the_original_consumer_error() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = crate::shop_items_fixture::write_archive(directory.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let budget = StarmapTraversalBudget::default();
    let entry = DataForgeEntry::read(&archive)?;
    let mut calls = 0;
    let result = visit_shops_from(&archive, &entry, budget, None, |_| {
        calls += 1;
        Err(ConsumerError::Stop(calls))
    });
    assert!(matches!(result, Err(ConsumerError::Stop(1))));
    assert_eq!(calls, 1, "a consumer error stops the visit");
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = visit_shops::<ConsumerError>(&archive, budget, Some(&cancel), |_| {
        panic!("a cancelled visit must never start a callback")
    });
    assert!(matches!(
        result,
        Err(ConsumerError::Query(CatalogError::Cancelled))
    ));
    Ok(())
}

#[test]
fn registry_availability_distinguishes_missing_invalid_and_valid_empty() -> TestResult {
    let directory = tempfile::tempdir()?;
    let cases = [
        (None, "absent", true),
        (Some("not XML"), "unavailable", false),
        (Some("<BackupData/>"), "available", true),
    ];
    for (registry, expected, complete) in cases {
        let path = directory.path().join(format!("{expected}.p4k"));
        let mut builder =
            ArchiveBuilder::new().entry(TestEntry::stored("Data/unrelated.txt", "data"));
        if let Some(registry) = registry {
            builder = builder.entry(TestEntry::stored(REGISTRY, registry));
        }
        builder.write_to(&path)?;
        let summary = visit_shops::<CatalogError>(
            &Archive::open(path)?,
            StarmapTraversalBudget::default(),
            None,
            |_| panic!("the fixture contains no admitted Shop"),
        )?;
        assert_eq!(serde_json::to_value(summary.availability)?, expected);
        assert_eq!(summary.coverage.registry_complete, complete);
        assert_eq!(summary.row_count, 0);
        assert!(summary.coverage.names.is_none());
        assert!(summary.coverage.declarations_complete.is_none());
        assert_eq!(summary.coverage.inventory_payloads, 0);
    }
    Ok(())
}
