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

#[test]
fn two_neutral_reserves_cannot_replace_each_other_at_the_same_sequence() -> TestResult {
    let memory = tests::memory(crate::installer::raw_path::RawPath::Absent);
    let mut winner = tests::memory(memory.state.path.clone());
    reservation::create(&mut winner, tests::sealing::identity(9)?)?;
    let expected = winner.state.clone();
    let mut race = Race {
        memory,
        interleave: Some(expected.clone()),
    };
    assert_eq!(
        reservation::create(&mut race, tests::sealing::identity(10)?),
        Err(Fault::Conflict),
        "the full snapshot CAS admits exactly one fresh nonce"
    );
    assert_eq!(
        race.memory.state, expected,
        "the winning neutral reservation is retained"
    );
    Ok(())
}

#[test]
fn recovery_and_seal_compete_on_the_same_complete_snapshot() -> TestResult {
    for recovery_wins in [false, true] {
        let (memory, binding) = tests::sealing::neutral()?;
        let mut winner = tests::memory(memory.state.path.clone());
        winner.state = memory.state.clone();
        let nonce = InvocationNonce::from_bytes([10; 32])?;
        if recovery_wins {
            reservation::recover(&mut winner, binding, nonce)?;
        } else {
            reservation::seal(&mut winner, binding, tests::sealing::declaration()?)?;
        }
        let expected = winner.state.clone();
        let mut race = Race {
            memory,
            interleave: Some(expected.clone()),
        };
        let result = if recovery_wins {
            reservation::seal(&mut race, binding, tests::sealing::declaration()?)
        } else {
            reservation::recover(&mut race, binding, nonce).map(|_| ())
        };
        assert_eq!(
            result,
            Err(Fault::Conflict),
            "a stale phase transition cannot overwrite the winner"
        );
        assert_eq!(
            race.memory.state, expected,
            "phase, nonce and full terminal evidence stay together"
        );
    }
    Ok(())
}

#[test]
fn a_foreign_path_edit_winning_seal_cas_requires_a_fresh_read_without_losing_authority()
-> TestResult {
    let (mut memory, binding) = tests::sealing::neutral()?;
    let foreign = tests::path(
        "foreign;;A;",
        crate::installer::raw_path::StringKind::Expandable,
    );
    memory.interleave = Some(foreign.clone());
    assert_eq!(
        reservation::seal(&mut memory, binding, tests::sealing::declaration()?),
        Err(Fault::Conflict),
        "seal compares the complete observed native image"
    );
    assert_eq!(
        memory.state.path, foreign,
        "legitimate foreign bytes win unchanged"
    );
    reservation::seal(&mut memory, binding, tests::sealing::declaration()?)?;
    transaction::begin_reserved(&mut memory, binding)?;
    assert_eq!(
        memory
            .state
            .journal
            .as_ref()
            .map(|record| &record.before_path),
        Some(&foreign),
        "the later prepare snapshots those actual bytes"
    );
    Ok(())
}
