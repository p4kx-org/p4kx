// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::journal_codec;
use crate::installer::journal_encode;
use crate::installer::path_edit;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::state::{Phase, Request, Snapshot};
use crate::installer::tests::{self, ENTRY, PRODUCT, TestResult};

fn repaired(original: RawPath, phase: Phase) -> Result<Snapshot, Fault> {
    let mut store = tests::memory(original);
    tests::install(&mut store)?;
    let id = tests::begin(&mut store, Request::Repair { product: PRODUCT })?;
    if phase != Phase::Prepared {
        tests::apply(&mut store, id)?;
    }
    match phase {
        Phase::Committed => tests::commit(&mut store, id)?,
        Phase::RolledBack => tests::rollback(&mut store, id)?,
        Phase::Prepared | Phase::Applied => {}
    }
    Ok(store.state)
}

fn reject_swap(phase: Phase) -> TestResult {
    let mut snapshot = repaired(tests::path(ENTRY, StringKind::Plain), phase)?;
    let owner = snapshot.owner.as_mut().ok_or("missing borrowed owner")?;
    assert_eq!(owner.claim, Claim::Borrowed, "the fixture borrows PATH");
    owner.claim = Claim::Inserted(path_edit::insert(&RawPath::Absent, &owner.lease)?);
    assert_corrupt(&snapshot, phase)
}

fn assert_corrupt(snapshot: &Snapshot, phase: Phase) -> TestResult {
    assert_eq!(
        journal_codec::encode(snapshot),
        Err(Fault::CorruptState),
        "{phase:?} cannot turn a foreign borrowed entry into owned bytes"
    );
    let malformed = journal_encode::snapshot(snapshot)?;
    assert_eq!(
        journal_codec::decode(&malformed),
        Err(Fault::CorruptState),
        "{phase:?} cannot recover a claim that disagrees with its receipt"
    );
    Ok(())
}

#[test]
fn prepared_borrowed_claim_cannot_become_an_insertion() -> TestResult {
    reject_swap(Phase::Prepared)
}

#[test]
fn applied_borrowed_claim_cannot_become_an_insertion() -> TestResult {
    reject_swap(Phase::Applied)
}

#[test]
fn committed_borrowed_claim_cannot_become_an_insertion() -> TestResult {
    reject_swap(Phase::Committed)
}

#[test]
fn rolled_back_borrowed_claim_cannot_become_an_insertion() -> TestResult {
    reject_swap(Phase::RolledBack)
}

#[test]
fn every_phase_rejects_swapping_one_valid_inserted_proof_for_another() -> TestResult {
    for phase in [
        Phase::Prepared,
        Phase::Applied,
        Phase::Committed,
        Phase::RolledBack,
    ] {
        let mut snapshot = repaired(tests::path("A", StringKind::Plain), phase)?;
        let owner = snapshot.owner.as_mut().ok_or("missing inserted owner")?;
        assert!(
            matches!(owner.claim, Claim::Inserted(_)),
            "the fixture owns its original insertion"
        );
        owner.claim = Claim::Inserted(path_edit::insert(
            &tests::path("UNRELATED", StringKind::Expandable),
            &owner.lease,
        )?);
        assert_corrupt(&snapshot, phase)?;
    }
    Ok(())
}
