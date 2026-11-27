// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutResolver};
use p4kx_extraction::{Archive, DataForge};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;

#[path = "file_proofs_context_fixture.rs"]
mod fixture;

use fixture::Probe;

#[test]
fn actual_context_budget_receipt_without_a_parse_survives_finish_without_replay() -> TestResult {
    ship_pool_test_input::with_input(&fixture::database(), fixture::entries(), check)
}

fn check<'input>(
    archive: &'input Archive,
    files: &'input ArchiveFiles<'input>,
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
) -> TestResult {
    let record = forge.record(0).ok_or("actual vehicle Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the actual original Record/Tree pair"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "no source decoder failure masks the file contract"
    );
    let source = crate::export_field_walk::inspect(
        &tree,
        &record,
        Some(Kind::Ships),
        ExportSourceRole::Selected,
        None,
    )?;
    let capture = Arc::new(Capture::for_input(input, &source, None, Kind::Ships));
    let probe = Arc::new(Probe::new(Arc::clone(&capture)));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), probe.clone())
            .with_files(files);
    resolver.resolve(&record)?;
    let first = capture.finish()?;
    first_delta(&first);
    let incomplete = first_counts(&probe)?;
    resolver.resolve(&record)?;
    let second = capture.finish()?;
    assert!(
        second.entries.is_empty(),
        "only genuine cache reuse occurred after the first delta drained"
    );
    retained_receipts(archive, &probe, incomplete)?;
    first_delta(&first);
    Ok(())
}

fn first_delta(first: &ExportLedger) {
    assert_eq!(
        first.entries.len(),
        3,
        "two genuine reads and one actual base parse"
    );
    for operation in [
        EntryOperation::VehicleImplementationBytes,
        EntryOperation::VehiclePatchBytes,
        EntryOperation::VehicleImplementation,
    ] {
        assert_eq!(
            first
                .entries
                .iter()
                .filter(|entry| entry.operation == operation)
                .count(),
            1,
            "each actual first file operation is retained once"
        );
    }
    assert!(
        !first
            .entries
            .iter()
            .any(|entry| entry.operation == EntryOperation::VehiclePatch),
        "context budget refusal cannot invent an intrinsic patch parse result"
    );
    assert!(
        first.incomplete_inventories.contains(&Kind::Ships),
        "the real context refusal retains its incomplete inventory"
    );
}

fn first_counts(probe: &Probe<'_>) -> Result<usize, Box<dyn Error>> {
    let seen = probe.seen.lock().map_err(|_| "actual observation mutex")?;
    assert_eq!(seen.reads, 2, "both original documents were actually read");
    assert_eq!(
        seen.projections, 1,
        "only the base reached intrinsic projection"
    );
    let incomplete = seen.incomplete;
    assert!(
        incomplete > 0,
        "the existing actual context budget refused patch parsing"
    );
    assert!(
        seen.reused.is_empty(),
        "the first real population is not a fabricated cache event"
    );
    drop(seen);
    Ok(incomplete)
}

fn retained_receipts(archive: &Archive, probe: &Probe<'_>, first_incomplete: usize) -> TestResult {
    let seen = probe.seen.lock().map_err(|_| "actual observation mutex")?;
    assert_eq!(seen.reads, 2, "the real cache performs no more byte reads");
    assert_eq!(
        seen.projections, 1,
        "the real cache performs no invented parse"
    );
    assert_eq!(
        seen.incomplete, first_incomplete,
        "a cache hit does not replay the first incomplete event"
    );
    assert_eq!(
        seen.reused.len(),
        2,
        "both original immutable receipts reach the actual Capture"
    );
    let patch = seen
        .reused
        .iter()
        .find(|receipt| receipt.kind() == LoadoutDocumentKind::VehiclePatch)
        .ok_or("actual retained patch receipt")?;
    assert!(
        patch.read_outcome().is_ok(),
        "the original patch read really succeeded"
    );
    assert!(
        patch.projection_outcome().is_none(),
        "its original intrinsic parse genuinely did not occur"
    );
    let origin = patch
        .source()
        .entry_origin()
        .ok_or("actual original patch Entry")?;
    assert!(
        archive.owns_origin(origin),
        "this exact Archive owns the retained source admission"
    );
    assert_eq!(
        origin.entry().path(),
        format!("Data/{}", fixture::PATCH),
        "the actual original patch path"
    );
    let base = seen
        .reused
        .iter()
        .find(|receipt| receipt.kind() == LoadoutDocumentKind::VehicleImplementation)
        .ok_or("actual retained base receipt")?;
    assert_eq!(base.read_outcome(), Ok(()), "the real original base read");
    assert_eq!(
        base.projection_outcome(),
        Some(Ok(())),
        "the real original base parse"
    );
    drop(seen);
    Ok(())
}
