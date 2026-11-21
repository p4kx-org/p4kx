// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, LoadoutFileError, LoadoutFiles};

use super::FileProofs;
use crate::ExportAccountingError;

#[path = "file_proofs_fixture.rs"]
mod fixture;

use fixture::TestResult;

#[test]
fn genuine_success_failure_and_missing_receipts_require_the_original_operations() -> TestResult {
    for contents in [
        Some(b"<Loadout/>".as_slice()),
        Some(b"<broken>".as_slice()),
        None,
    ] {
        let (_directory, archive) = fixture::archive(contents)?;
        let original = fixture::observe(&ArchiveFiles::new(&archive))?;
        assert!(
            !FileProofs::default().contains(&original.original),
            "an unobserved genuine receipt is not self-authorizing"
        );
        let proofs = original.populate()?;
        assert!(
            proofs.contains(&original.original),
            "the exact original operation pair authorizes reuse"
        );
    }
    Ok(())
}

#[test]
fn genuine_archive_receipt_rejects_a_foreign_input_with_equal_descriptors() -> TestResult {
    let (_left_directory, left) = fixture::archive(Some(b"<Loadout/>"))?;
    let (_right_directory, right) = fixture::archive(Some(b"<Loadout/>"))?;
    let original = fixture::observe(&ArchiveFiles::new(&left))?;
    let foreign = fixture::observe(&ArchiveFiles::new(&right))?;
    assert_eq!(
        original.original.source().identity(),
        foreign.original.source().identity(),
        "both genuine Archives deliberately have equal path/index descriptions"
    );
    assert!(
        !original
            .original
            .source()
            .is_same_admission(foreign.original.source()),
        "the real original input owners are nevertheless different"
    );
    let mut proofs = original.populate()?;
    assert!(
        proofs.contains(&original.original),
        "the cold original remains admitted"
    );
    assert!(
        !proofs.contains(&foreign.original),
        "equal descriptors do not authorize the foreign receipt"
    );
    assert!(
        matches!(
            proofs.projected(&foreign.original),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "a foreign parse has no observed original read"
    );
    assert!(
        proofs.contains(&original.original),
        "rejected foreign evidence cannot damage the original proof"
    );
    Ok(())
}

struct Generic;

impl LoadoutFiles for Generic {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(path, fixture::SELECTOR, "the original authored selector");
        Ok(b"<Loadout/>".to_vec())
    }
}

#[test]
fn genuine_unbound_receipt_accepts_its_whole_clone_but_not_a_fresh_equal_path() -> TestResult {
    let original = fixture::observe(&Generic)?;
    let same = original.original.clone();
    let proofs = original.populate()?;
    assert!(
        proofs.contains(&same),
        "a whole original receipt clone preserves actual admission"
    );
    assert!(
        same.is_same_operation(&original.original),
        "the genuine clone is the same operation"
    );
    let fresh = fixture::observe(&Generic)?;
    assert_eq!(
        fresh.original.source().identity(),
        same.source().identity(),
        "equal unbound descriptors"
    );
    assert!(
        !fresh.original.source().is_same_admission(same.source()),
        "fresh generic reads have different original handles"
    );
    let foreign = fresh.populate()?;
    assert!(
        !foreign.contains(&same),
        "a new equal-text unbound operation has no original handle"
    );
    Ok(())
}

#[test]
fn exact_intrinsic_kind_read_and_projection_outcomes_are_required() -> TestResult {
    let (_directory, archive) = fixture::archive(Some(b"<Loadout/>"))?;
    let files = ArchiveFiles::new(&archive);
    let original = fixture::observe(&files)?;
    let other_kind = fixture::observe_vehicle(&files)?;
    assert!(
        original
            .original
            .source()
            .is_same_admission(other_kind.original.source()),
        "two real intrinsic readers deliberately select the same physical Entry"
    );
    assert_ne!(
        original.original.kind(),
        other_kind.original.kind(),
        "the actual intrinsic operations differ"
    );
    assert_eq!(
        original.original.read_outcome(),
        other_kind.original.read_outcome(),
        "both real byte reads succeed"
    );
    assert!(
        other_kind
            .original
            .projection_outcome()
            .is_some_and(|value| value.is_err()),
        "the real vehicle parser rejects the stored Loadout root"
    );
    assert!(
        !other_kind.populate()?.contains(&original.original),
        "another intrinsic operation cannot authorize this receipt"
    );
    pending_projection(&original)?;
    Ok(())
}

fn pending_projection(original: &fixture::Witness) -> TestResult {
    let receipt = &original.original;
    let mut pending = FileProofs::default();
    original.read_into(&mut pending)?;
    assert!(
        !pending.contains(receipt),
        "an observed read cannot substitute for this receipt's actual parse callback"
    );
    pending.projected(receipt)?;
    assert!(
        pending.contains(receipt),
        "the original read and actual projection phases are both observed"
    );
    Ok(())
}

#[test]
fn exact_original_projection_errors_are_not_coarsened() -> TestResult {
    let (_directory, archive) = fixture::archive(Some(b"<broken>"))?;
    let files = ArchiveFiles::new(&archive);
    let original = fixture::observe(&files)?;
    let distinct = fixture::observe(&files)?;
    assert!(
        original
            .original
            .projection_outcome()
            .is_some_and(|value| value.is_err()),
        "the genuine XML parser supplies the original error"
    );
    assert_eq!(
        original.original.projection_outcome(),
        distinct.original.projection_outcome(),
        "the actual independent parser failures deliberately have identical full parameters"
    );
    assert!(
        !original.original.is_same_operation(&distinct.original),
        "equal errors remain distinct original operations"
    );
    let proofs = distinct.populate()?;
    assert!(
        !proofs.contains(&original.original),
        "even an identical failure outcome cannot replace the original operation"
    );
    assert!(
        original.populate()?.contains(&original.original),
        "the exact original malformed pair remains valid"
    );
    Ok(())
}

#[test]
fn projection_cannot_precede_its_actual_read_or_be_registered_twice() -> TestResult {
    let (_directory, archive) = fixture::archive(Some(b"<Loadout/>"))?;
    let original = fixture::observe(&ArchiveFiles::new(&archive))?;
    let receipt = &original.original;
    let mut proofs = FileProofs::default();
    assert!(
        matches!(
            proofs.projected(receipt),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "a parse cannot authorize its own absent byte callback"
    );
    original.read_into(&mut proofs)?;
    proofs.projected(receipt)?;
    assert!(
        proofs.contains(receipt),
        "one exact real pair is the positive control"
    );
    assert!(
        matches!(
            proofs.projected(receipt),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "one actual byte read cannot fabricate a second parse"
    );
    assert!(
        proofs.contains(receipt),
        "a rejected duplicate leaves the original pair intact"
    );
    Ok(())
}

pub(super) struct Unreadable {
    pub(super) reason: &'static str,
}

impl LoadoutFiles for Unreadable {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(path, fixture::SELECTOR, "the original failing selector");
        Err(LoadoutFileError::Unreadable {
            reason: self.reason.to_owned(),
        })
    }
}

#[test]
fn exact_original_read_error_parameters_remain_required() -> TestResult {
    let original = fixture::observe(&Unreadable {
        reason: "actual provider fault",
    })?;
    let receipt = &original.original;
    assert_eq!(
        receipt.read_outcome(),
        Err(&LoadoutFileError::Unreadable {
            reason: "actual provider fault".to_owned()
        }),
        "the genuine provider error is retained exactly"
    );
    assert!(
        receipt.projection_outcome().is_none(),
        "a failed byte read has no invented parser result"
    );
    assert!(
        original.populate()?.contains(receipt),
        "the exact original read failure authorizes its reuse"
    );
    let different = fixture::observe(&Unreadable {
        reason: "different provider fault",
    })?;
    assert_eq!(
        different.original.read_outcome(),
        Err(&LoadoutFileError::Unreadable {
            reason: "different provider fault".to_owned()
        }),
        "the independent actual failure keeps its full parameter"
    );
    let mut proofs = different.populate()?;
    assert!(
        !proofs.contains(receipt),
        "another actual failure cannot replace the exact original operation"
    );
    assert!(
        matches!(
            proofs.projected(&different.original),
            Err(ExportAccountingError::UnobservedFailure)
        ),
        "the actually failed byte operation cannot authorize a parse"
    );
    Ok(())
}

#[path = "file_proofs_sequence_tests.rs"]
mod sequence;

#[path = "file_proofs_context_tests.rs"]
mod context;
