// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, LoadoutReadEvent, LoadoutReadObserver};

use super::FileProofs;
use super::Unreadable;
use super::fixture::{self, TestResult, Witness};
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;
use crate::{CatalogError, ExportAccountingError};

struct Originals {
    older: Witness,
    newer: Witness,
}

fn originals() -> Result<Originals, Box<dyn std::error::Error>> {
    let (_directory, archive) = fixture::archive(Some(b"<Loadout/>"))?;
    let older = fixture::observe(&ArchiveFiles::new(&archive))?;
    let newer = fixture::observe(&ArchiveFiles::new(&archive))?;
    assert!(
        older
            .original
            .source()
            .is_same_admission(newer.original.source()),
        "two genuine cold readers admit the same physical original Entry"
    );
    assert!(
        !older.original.is_same_operation(&newer.original),
        "independent real cold reads are distinct operations"
    );
    assert_eq!(
        older.original.kind(),
        newer.original.kind(),
        "the same actual intrinsic document operation"
    );
    for witness in [&older, &newer] {
        assert!(
            witness.original.read_outcome().is_ok(),
            "each actual original byte read completed"
        );
        assert_eq!(
            witness.original.projection_outcome(),
            Some(Ok(())),
            "each actual original parse completed"
        );
        assert!(
            witness.populate()?.contains(&witness.original),
            "each unchanged genuine pair is admitted"
        );
    }
    Ok(Originals { older, newer })
}

#[test]
fn a_failed_operation_cannot_borrow_an_older_pending_read() -> TestResult {
    let originals = originals()?;
    let failed = fixture::observe(&Unreadable {
        reason: "actual later provider fault",
    })?;
    assert!(
        failed.original.read_outcome().is_err(),
        "the later actual operation really failed"
    );
    let mut proofs = FileProofs::default();
    originals.older.read_into(&mut proofs)?;
    failed.read_into(&mut proofs)?;
    assert!(
        matches!(
            proofs.projected(&failed.original),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "the failed operation cannot take another original operation's pending read"
    );
    assert!(
        proofs.contains(&failed.original),
        "the real failed read still supplies its exact immutable cache receipt"
    );
    proofs.projected(&originals.older.original)?;
    assert!(
        proofs.contains(&originals.older.original),
        "an older real operation may finish after another actual read; chronology is not authority"
    );
    Ok(())
}

#[test]
fn a_duplicate_projection_cannot_borrow_another_pending_read() -> TestResult {
    let originals = originals()?;
    let mut proofs = FileProofs::default();
    originals.older.read_into(&mut proofs)?;
    originals.newer.read_into(&mut proofs)?;
    proofs.projected(&originals.newer.original)?;
    assert!(
        proofs.contains(&originals.newer.original),
        "the real younger completed receipt is a positive control"
    );
    assert!(
        matches!(
            proofs.projected(&originals.newer.original),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "a duplicate of the younger operation cannot consume the older pending read"
    );
    proofs.projected(&originals.older.original)?;
    assert!(
        proofs.contains(&originals.older.original),
        "the correct older receipt remains independently projectable"
    );
    assert!(
        proofs.contains(&originals.newer.original),
        "rejecting the duplicate preserves the actual younger receipt"
    );
    Ok(())
}

#[test]
fn an_unobserved_same_admission_operation_cannot_alias_an_observed_read() -> TestResult {
    let originals = originals()?;
    assert_eq!(
        originals.older.original.read_outcome(),
        originals.newer.original.read_outcome(),
        "the actual byte outcomes deliberately agree"
    );
    assert_eq!(
        originals.older.original.projection_outcome(),
        originals.newer.original.projection_outcome(),
        "the actual intrinsic outcomes deliberately agree"
    );
    let mut proofs = FileProofs::default();
    originals.older.read_into(&mut proofs)?;
    assert!(
        matches!(
            proofs.projected(&originals.newer.original),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "equal source/kind/outcomes do not establish the original operation"
    );
    assert!(
        !proofs.contains(&originals.newer.original),
        "the different real operation has no observed read"
    );
    proofs.projected(&originals.older.original)?;
    assert!(
        proofs.contains(&originals.older.original),
        "the correct original receipt still completes"
    );
    Ok(())
}

#[test]
fn a_cloned_read_receipt_is_not_another_io_operation_or_ledger_row() -> TestResult {
    let originals = originals()?;
    let receipt = &originals.older.original;
    let request = p4kx_assembly::LoadoutPath::unbound(fixture::SELECTOR);
    let caller = p4kx_assembly::LoadoutRequest::Xml {
        path: request.clone(),
    };
    let capture = Capture::new(&ExportLedger::default(), None);
    assert!(
        capture
            .observe(LoadoutReadEvent::FileRead {
                observation: receipt,
                request: &request,
                caller: &caller
            })
            .is_continue(),
        "the actual original read callback is observed once"
    );
    let first = capture.finish()?;
    assert_eq!(
        first.entries.len(),
        1,
        "one original byte-read operation is retained"
    );
    let cloned = receipt.clone();
    assert!(
        receipt.is_same_operation(&cloned),
        "a whole clone keeps the genuine operation"
    );
    assert!(
        capture
            .observe(LoadoutReadEvent::FileRead {
                observation: &cloned,
                request: &request,
                caller: &caller
            })
            .is_break(),
        "a repeated read callback cannot invent another IO operation"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::UnobservedFailure
            ))
        ),
        "duplicate read registration is an explicit accounting failure"
    );
    let state = capture.lock()?;
    assert!(
        state.ledger.entries.is_empty(),
        "the rejected duplicate added no row after the first delta drained"
    );
    drop(state);
    assert_eq!(
        first.entries.len(),
        1,
        "the first genuine operation remains unchanged"
    );
    Ok(())
}
