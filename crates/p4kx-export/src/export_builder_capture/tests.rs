// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, TreeBuilder};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use super::BuilderCapture;
use crate::{CatalogError, ExportAccountingError, Kind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Source"))
        .record(TestRecord::new(1, "Source", "Source.One", "source/one.xml"))
        .build()
}

#[test]
fn option_helpers_cannot_swallow_observer_cancellation() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let cancel = CancelToken::new();
    let capture = Arc::new(BuilderCapture::for_kind(Kind::Items, Some(&cancel)));
    let mut trees = TreeBuilder::with_read_observer(capture.clone());
    cancel.cancel();
    assert!(
        trees.build(&record).ok().is_none(),
        "an optional helper can consume the typed stop sentinel"
    );
    assert!(
        matches!(capture.finish(), Err(CatalogError::Cancelled)),
        "the original cancellation is returned before any mapped rows are accepted"
    );
    Ok(())
}

#[test]
fn an_original_accounting_error_is_not_relabelled_as_invalid_source_bytes() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let capture = Arc::new(BuilderCapture::for_kind(Kind::Items, None));
    capture.state.lock().map_err(|_| "capture lock")?.fatal =
        Some(ExportAccountingError::CounterOverflow.into());
    let mut trees = TreeBuilder::with_read_observer(capture.clone());
    assert!(
        matches!(
            trees.build(&record),
            Err(DataForgeError::ObservationStopped)
        ),
        "the format layer returns only the typed caller stop"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::CounterOverflow
            ))
        ),
        "the original accounting error survives the Option-helper boundary"
    );
    Ok(())
}

#[test]
fn parallel_captures_keep_their_actual_source_contexts_separate() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let read = |kind| -> Result<_, CatalogError> {
        let capture = Arc::new(BuilderCapture::for_kind(kind, None));
        let mut trees = TreeBuilder::with_read_observer(capture.clone());
        trees.build(&record).map_err(CatalogError::DataForge)?;
        capture.finish()
    };
    let (items, ships) = rayon::join(|| read(Kind::Items), || read(Kind::Ships));
    let items = items?;
    let ships = ships?;
    assert_eq!(
        items.sources.len(),
        1,
        "one actual dependency was read in the item context"
    );
    assert_eq!(
        ships.sources.len(),
        1,
        "one actual dependency was read in the ship context"
    );
    assert!(
        items
            .sources
            .iter()
            .all(|source| source.kind == Some(Kind::Items)),
        "parallel item context is not overwritten"
    );
    assert!(
        ships
            .sources
            .iter()
            .all(|source| source.kind == Some(Kind::Ships)),
        "parallel ship context is not overwritten"
    );
    assert_eq!(
        items
            .field_inspections
            .first()
            .ok_or("item proof")?
            .record_index,
        record.index(),
        "the proof names the actual physical source"
    );
    Ok(())
}

#[test]
fn poisoned_evidence_stops_optional_helpers_and_refuses_publication() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let capture = Arc::new(BuilderCapture::for_kind(Kind::Items, None));
    assert!(
        crate::export_evidence_lock::poison(&capture.state),
        "the fixture poisons actual evidence state"
    );
    let mut trees = TreeBuilder::with_read_observer(capture.clone());
    assert!(
        trees.build(&record).ok().is_none(),
        "a poisoned observer stops even through an optional helper"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::PoisonedEvidence
            ))
        ),
        "poison is a typed accounting failure rather than accepted partial evidence"
    );
    Ok(())
}

#[test]
fn poisoned_selection_context_cannot_be_activated_or_finished() {
    let capture = BuilderCapture::for_root(0, None);
    assert!(
        crate::export_evidence_lock::poison(&capture.context),
        "the fixture poisons actual source context"
    );
    assert!(
        matches!(
            capture.activate(Kind::Items),
            Err(CatalogError::Accounting(
                ExportAccountingError::PoisonedEvidence
            ))
        ),
        "selection cannot read or overwrite a poisoned context"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::PoisonedEvidence
            ))
        ),
        "empty evidence cannot hide a poisoned selection context"
    );
}

#[test]
fn a_known_original_fatal_is_preserved_when_a_separate_context_is_poisoned() -> TestResult {
    let capture = BuilderCapture::for_kind(Kind::Items, None);
    capture
        .state
        .lock()
        .map_err(|_| "capture fixture lock")?
        .fatal = Some(ExportAccountingError::CounterOverflow.into());
    assert!(
        crate::export_evidence_lock::poison(&capture.context),
        "the separate context is poisoned after an original failure was captured"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::CounterOverflow
            ))
        ),
        "sound captured fatal evidence keeps its original typed failure"
    );
    Ok(())
}
