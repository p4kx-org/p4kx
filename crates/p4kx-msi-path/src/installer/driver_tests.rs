// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use crate::installer::arguments::{Command, Invocation, Stage};
use crate::installer::binding::Binding;
use crate::installer::caller::{Caller, Goal};
use crate::installer::driver::{self, DriverError};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::{InvocationNonce, NonceError};
use crate::installer::journal_codec;
use crate::installer::notification::{Failure, FailureKind, Report, Stop};
use crate::installer::raw_path::StringKind;
use crate::installer::state::Phase;
use crate::installer::tests::{self, Memory, TestResult};

fn caller(byte: u8) -> Result<Caller, Fault> {
    Ok(Caller {
        product: 1,
        package: 101,
        nonce: InvocationNonce::from_bytes([byte; 32])?,
        predecessor: None,
        goal: Goal::Ensure(tests::lease()?),
    })
}

fn request(stage: Stage, expected: u64, caller: &Caller) -> Command {
    Command::Transaction(Invocation {
        stage,
        binding: Binding::for_caller(expected, caller),
    })
}

const fn no_random() -> Result<InvocationNonce, NonceError> {
    Err(NonceError::ReservedValue)
}

fn complete() -> Report {
    Report {
        stop: Stop::Complete,
        acknowledged: 1,
        vanished: 0,
        excluded_own_process: 0,
        timed_out: 0,
    }
}

fn run(
    store: &mut Memory,
    stage: Stage,
    expected: u64,
    caller: &Caller,
) -> Result<(), DriverError<Fault>> {
    if stage == Stage::Prepare
        && store.state.sequence == expected
        && store.state.reservation.is_none()
    {
        tests::reservation::reserve(store, caller.clone()).map_err(DriverError::Store)?;
    }
    driver::execute(store, request(stage, expected, caller), no_random, || {
        Ok(complete())
    })
}

#[test]
fn phase_boundary_persists_receipt_before_path_and_notifies_only_changes() -> TestResult {
    let original = tests::path(";;A;;", StringKind::Expandable);
    let mut store = tests::memory(original.clone());
    let caller = caller(1)?;
    let notifications = Cell::new(0);
    tests::reservation::reserve(&mut store, caller.clone())?;
    for stage in [Stage::Preflight, Stage::Prepare, Stage::Prepare] {
        driver::execute(&mut store, request(stage, 0, &caller), no_random, || {
            notifications.set(notifications.get() + 1);
            Ok(complete())
        })?;
        assert_eq!(
            store.state.path, original,
            "preparation never edits the source PATH"
        );
    }
    let encoded = journal_codec::encode(&store.state)?;
    store.state = journal_codec::decode(&encoded)?;
    assert_eq!(
        store.state.journal.as_ref().map(|record| &record.caller),
        Some(&caller),
        "durable reload retains the exact nonce-bound caller"
    );
    for stage in [Stage::Apply, Stage::Apply, Stage::Commit, Stage::Commit] {
        driver::execute(&mut store, request(stage, 0, &caller), no_random, || {
            notifications.set(notifications.get() + 1);
            Ok(complete())
        })?;
    }
    assert_eq!(
        notifications.get(),
        1,
        "only the actual PATH mutation sends a notification"
    );
    assert_eq!(
        store.state.journal.as_ref().map(|record| record.phase),
        Some(Phase::Committed),
        "the terminal proof remains available to the matching rollback callback"
    );
    Ok(())
}

#[test]
fn another_invocation_cannot_replay_any_phase_of_a_terminal_receipt() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Expandable));
    let original = caller(1)?;
    run(&mut store, Stage::Prepare, 0, &original)?;
    run(&mut store, Stage::Apply, 0, &original)?;
    run(&mut store, Stage::Commit, 0, &original)?;
    let before = store.state.clone();
    let later = caller(2)?;
    for stage in [Stage::Prepare, Stage::Apply, Stage::Commit, Stage::Rollback] {
        let result = run(&mut store, stage, 0, &later);
        assert!(
            matches!(result, Err(DriverError::Store(Fault::WrongTransaction))),
            "same product/package/sequence does not transfer a prior invocation's receipt"
        );
        assert_eq!(
            store.state, before,
            "failed replay cannot change PATH, phase or ownership"
        );
    }
    Ok(())
}

#[test]
fn competing_prepares_never_replace_the_first_active_binding() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    run(&mut store, Stage::Prepare, 0, &caller(1)?)?;
    let prepared = store.state.clone();
    assert!(
        matches!(
            run(&mut store, Stage::Prepare, 0, &caller(2)?),
            Err(DriverError::Store(Fault::WrongTransaction))
        ),
        "the first durable reservation owns its exact receipt"
    );
    assert_eq!(
        store.state, prepared,
        "a concurrent prepare cannot retarget AppSearch to another invocation"
    );
    Ok(())
}

#[test]
fn notification_failure_leaves_applied_wal_for_matching_compensation() -> TestResult {
    let original = tests::path(";A;;B;", StringKind::Expandable);
    let mut store = tests::memory(original.clone());
    let caller = caller(3)?;
    run(&mut store, Stage::Prepare, 0, &caller)?;
    let failure = Failure {
        kind: FailureKind::Native(5),
        progress: Report {
            acknowledged: 0,
            ..complete()
        },
    };
    let result = driver::execute(
        &mut store,
        request(Stage::Apply, 0, &caller),
        no_random,
        || Err(failure.clone()),
    );
    assert!(
        matches!(result, Err(DriverError::Notification(error)) if error.kind == FailureKind::Native(5)),
        "the actual notification failure is retained"
    );
    assert_eq!(
        store.state.journal.as_ref().map(|record| record.phase),
        Some(Phase::Applied),
        "failed notification cannot erase the already-applied operation"
    );
    run(&mut store, Stage::Rollback, 0, &caller)?;
    assert_eq!(
        store.state.path, original,
        "matching rollback restores exact original bytes and kind"
    );
    Ok(())
}

#[test]
fn incomplete_notification_is_not_reported_as_success() -> TestResult {
    let mut store = tests::memory(tests::path("A", StringKind::Plain));
    let caller = caller(4)?;
    run(&mut store, Stage::Prepare, 0, &caller)?;
    let result = driver::execute(
        &mut store,
        request(Stage::Apply, 0, &caller),
        no_random,
        || {
            Ok(Report {
                stop: Stop::WindowLimit,
                acknowledged: 1,
                vanished: 0,
                excluded_own_process: 0,
                timed_out: 0,
            })
        },
    );
    assert!(
        matches!(result, Err(DriverError::IncompleteNotification(report)) if report.stop == Stop::WindowLimit),
        "a bounded partial broadcast remains a failure requiring compensation"
    );
    Ok(())
}
