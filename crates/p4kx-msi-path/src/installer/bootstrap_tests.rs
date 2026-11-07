// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use crate::installer::arguments::{Command, Invocation, Stage};
use crate::installer::binding::Binding;
use crate::installer::caller::Goal;
use crate::installer::declaration::Declaration;
use crate::installer::driver::{self, DriverError};
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::{InvocationNonce, NonceError};
use crate::installer::notification::{Report, Stop};
use crate::installer::raw_path::StringKind;
use crate::installer::tests::{self, TestResult};

fn declaration() -> Result<Declaration, Fault> {
    Ok(Declaration {
        product: 1,
        package: 101,
        predecessor: None,
        goal: Goal::Ensure(tests::lease()?),
    })
}

const fn complete() -> Report {
    Report {
        stop: Stop::Complete,
        acknowledged: 1,
        vanished: 0,
        timed_out: 0,
    }
}
const fn no_random() -> Result<InvocationNonce, NonceError> {
    Err(NonceError::ReservedValue)
}

#[test]
fn native_command_shape_generates_random_only_once_then_reuses_the_same_captured_binding()
-> TestResult {
    let mut memory = tests::memory(tests::path(";A;;", StringKind::Expandable));
    let generated = InvocationNonce::from_bytes([31; 32])?;
    let random_calls = Cell::new(0);
    let notifications = Cell::new(0);
    driver::execute(
        &mut memory,
        Command::Reserve {
            product: 1,
            package: 101,
        },
        || {
            random_calls.set(random_calls.get() + 1);
            Ok(generated)
        },
        || {
            notifications.set(notifications.get() + 1);
            Ok(complete())
        },
    )?;
    let binding = memory
        .state
        .reservation
        .as_ref()
        .ok_or("missing reservation")?
        .binding();
    driver::execute(
        &mut memory,
        Command::Seal {
            binding,
            declaration: declaration()?,
        },
        no_random,
        || Ok(complete()),
    )?;
    for stage in [
        Stage::Preflight,
        Stage::Prepare,
        Stage::Apply,
        Stage::Commit,
        Stage::Rollback,
    ] {
        driver::execute(
            &mut memory,
            Command::Transaction(Invocation { stage, binding }),
            no_random,
            || {
                notifications.set(notifications.get() + 1);
                Ok(complete())
            },
        )?;
    }
    verify_phase_results(&memory, random_calls.get(), notifications.get(), binding)?;
    Ok(())
}

fn verify_phase_results(
    memory: &tests::Memory,
    random_calls: u32,
    notifications: u32,
    binding: Binding,
) -> TestResult {
    assert_eq!(
        random_calls, 1,
        "the phase callbacks cannot generate unrelated random receipts"
    );
    assert_eq!(
        notifications, 2,
        "only actual apply and rollback PATH changes notify"
    );
    let record = memory.state.journal.as_ref().ok_or("missing WAL")?;
    assert_eq!(
        Binding::for_caller(0, &record.caller),
        binding,
        "every phase retains Product/Package/N/nonce"
    );
    Ok(())
}

#[test]
fn random_source_failure_is_typed_and_changes_no_state() {
    let mut memory = tests::memory(tests::path("A", StringKind::Plain));
    let before = memory.state.clone();
    let result = driver::execute(
        &mut memory,
        Command::Reserve {
            product: 1,
            package: 101,
        },
        no_random,
        || Ok(complete()),
    );
    assert!(
        matches!(result, Err(DriverError::Random(NonceError::ReservedValue))),
        "random failure is not relabeled Registry or ownership failure"
    );
    assert_eq!(
        memory.state, before,
        "no reservation or PATH write follows a random-source refusal"
    );
}

#[test]
fn explicit_recovery_uses_one_new_nonce_without_replacing_the_authored_goal() -> TestResult {
    let mut memory = tests::memory(tests::path("A", StringKind::Plain));
    let first_nonce = InvocationNonce::from_bytes([31; 32])?;
    driver::execute(
        &mut memory,
        Command::Reserve {
            product: 1,
            package: 101,
        },
        || Ok(first_nonce),
        || Ok(complete()),
    )?;
    let old = memory
        .state
        .reservation
        .as_ref()
        .ok_or("missing reservation")?
        .clone();
    driver::execute(
        &mut memory,
        Command::Seal {
            binding: old.binding(),
            declaration: declaration()?,
        },
        no_random,
        || Ok(complete()),
    )?;
    let old = memory
        .state
        .reservation
        .as_ref()
        .ok_or("missing sealed reservation")?
        .clone();
    let new_nonce = InvocationNonce::from_bytes([32; 32])?;
    driver::execute(
        &mut memory,
        Command::Recover(old.binding()),
        || Ok(new_nonce),
        || Ok(complete()),
    )?;
    let mut expected = old;
    expected.set_nonce(new_nonce);
    assert_eq!(
        memory.state.reservation.as_ref(),
        Some(&expected),
        "recovery changes only the nonce of that exact authored declaration"
    );
    Ok(())
}
