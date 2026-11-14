// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogOptions, build_catalog, build_catalog_from};
use p4kx_extraction::CancelToken;

use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use crate::health_snapshot_support as support;

#[test]
fn actual_catalog_summary_clones_share_one_immutable_result_owned_inventory() -> TestResult {
    let source = Fixture::open(
        &fixture::broken_lists(1, false),
        Input::Encrypted,
        Vec::new(),
    )?;
    support::broken_subject(&source)?;
    let (built, catalog) = source.diagnostic()?;
    let inventory = built.summary().source_inventory();
    support::closed(inventory);
    assert_eq!(
        catalog.get("provenance"),
        Some(&serde_json::to_value(inventory)?),
        "serialized catalog and actual summary retain the same snapshot contents"
    );
    let cloned = built.summary().clone();
    assert!(
        std::ptr::eq(inventory, cloned.source_inventory()),
        "public summary cloning shares the existing immutable inventory allocation"
    );
    assert_eq!(
        inventory,
        cloned.source_inventory(),
        "shared immutable snapshot values remain structurally equal"
    );
    drop(built);
    drop(source);
    support::closed(cloned.source_inventory());
    Ok(())
}

#[test]
fn actual_normal_refusal_retains_its_own_inventory_and_unrelated_errors_do_not_invent_one()
-> TestResult {
    let source = Fixture::open(&fixture::broken_lists(1, false), Input::Plain, Vec::new())?;
    support::broken_subject(&source)?;
    let error = build_catalog_from(&source.archive, &source.entry, &fixture::options())
        .err()
        .ok_or("actual normal refusal")?;
    let CatalogError::IncompleteExport(refusal) = &error else {
        return Err("the genuine source failure remains typed IncompleteExport".into());
    };
    let health = refusal.health();
    assert_eq!(
        health.failed_records().len(),
        1,
        "the actual normal report retains its failed physical subject"
    );
    support::closed(error.source_inventory().ok_or("actual refusal snapshot")?);
    let token = CancelToken::new();
    token.cancel();
    let cancelled = build_catalog(
        &source.archive,
        &CatalogOptions {
            cancel: Some(&token),
            ..fixture::options()
        },
    )
    .err()
    .ok_or("actual early cancellation")?;
    assert!(
        matches!(&cancelled, CatalogError::Cancelled),
        "the actual fatal cancellation keeps its original type"
    );
    assert!(
        cancelled.source_inventory().is_none(),
        "no unrelated error receives a fabricated refusal inventory"
    );
    Ok(())
}
