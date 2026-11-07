// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::reservation;
use crate::installer::state::{Observed, Snapshot, Store};
use crate::installer::tests::{self, Memory, TestResult};
use crate::installer::transaction;

struct Race {
    memory: Memory,
    interleave: Option<Snapshot>,
}

impl Store for Race {
    type Revision = u64;
    type Error = Fault;
    fn load(&self) -> Result<Observed<u64>, Fault> {
        self.memory.load()
    }
    fn compare_replace(
        &mut self,
        expected: &Observed<u64>,
        replacement: Snapshot,
    ) -> Result<(), Fault> {
        if let Some(interleave) = self.interleave.take() {
            self.memory.state = interleave;
        }
        self.memory.compare_replace(expected, replacement)
    }
}

#[test]
fn a_prepare_that_wins_the_cas_race_cannot_be_reclaimed_by_recovery() -> TestResult {
    let (memory, binding) = tests::reservation::reserved()?;
    let mut competitor = tests::memory(memory.state.path.clone());
    competitor.state = memory.state.clone();
    transaction::begin_reserved(&mut competitor, binding)?;
    let prepared = competitor.state;
    let mut race = Race {
        memory,
        interleave: Some(prepared.clone()),
    };
    assert_eq!(
        reservation::recover(&mut race, binding, InvocationNonce::from_bytes([10; 32])?),
        Err(Fault::Conflict),
        "the exact observed Reserved state must still exist at atomic replacement"
    );
    assert_eq!(
        race.memory.state, prepared,
        "recovery cannot overwrite a winning prepared WAL"
    );
    Ok(())
}

#[test]
fn a_recovery_that_wins_the_cas_race_invalidates_an_already_observed_prepare() -> TestResult {
    let (memory, binding) = tests::reservation::reserved()?;
    let mut competitor = tests::memory(memory.state.path.clone());
    competitor.state = memory.state.clone();
    reservation::recover(
        &mut competitor,
        binding,
        InvocationNonce::from_bytes([10; 32])?,
    )?;
    let recovered = competitor.state;
    let mut race = Race {
        memory,
        interleave: Some(recovered.clone()),
    };
    assert_eq!(
        transaction::begin_reserved(&mut race, binding),
        Err(Fault::Conflict),
        "prepare cannot consume a replaced reservation even after observing the old caller"
    );
    assert_eq!(
        race.memory.state, recovered,
        "the winning fresh nonce remains available"
    );
    assert_eq!(
        transaction::begin_reserved(&mut race, binding),
        Err(Fault::WrongTransaction),
        "all later old callbacks fail at identity validation"
    );
    Ok(())
}
#[test]
fn cancellation_cannot_erase_a_concurrently_materialised_wal() -> TestResult {
    let (memory, binding) = tests::reservation::reserved()?;
    let mut competitor = tests::memory(memory.state.path.clone());
    competitor.state = memory.state.clone();
    transaction::begin_reserved(&mut competitor, binding)?;
    let prepared = competitor.state;
    let mut race = Race {
        memory,
        interleave: Some(prepared.clone()),
    };
    assert_eq!(
        reservation::cancel(&mut race, binding),
        Err(Fault::Conflict),
        "cancel must still see the exact reserved state at commit"
    );
    assert_eq!(
        race.memory.state, prepared,
        "winning prepare is never replaced by a cancellation fence"
    );
    Ok(())
}

#[test]
fn a_winning_cancel_fences_an_already_observed_prepare_without_consuming_path() -> TestResult {
    let (memory, binding) = tests::reservation::reserved()?;
    let mut competitor = tests::memory(memory.state.path.clone());
    competitor.state = memory.state.clone();
    reservation::cancel(&mut competitor, binding)?;
    let cancelled = competitor.state;
    let mut race = Race {
        memory,
        interleave: Some(cancelled.clone()),
    };
    assert_eq!(
        transaction::begin_reserved(&mut race, binding),
        Err(Fault::Conflict),
        "prepare cannot consume cancelled authority from a stale read"
    );
    assert_eq!(
        race.memory.state, cancelled,
        "the exact cancellation fence remains durable"
    );
    Ok(())
}
