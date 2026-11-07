// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::installer::arguments::{Command, Invocation, Stage};
use crate::installer::fault::Fault;
use crate::installer::notification::{Failure, Report, Stop};
use crate::installer::state::{PathChange, Store};
use crate::installer::transaction;

#[derive(Debug)]
pub(crate) enum DriverError<StoreError> {
    Store(StoreError),
    Notification(Failure),
    IncompleteNotification(Report),
}

impl<StoreError: fmt::Display> fmt::Display for DriverError<StoreError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => fmt::Display::fmt(error, formatter),
            Self::Notification(error) => write!(
                formatter,
                "environment notification failed: {:?}",
                error.kind
            ),
            Self::IncompleteNotification(report) => write!(
                formatter,
                "environment notification incomplete: {:?}, {} timeouts",
                report.stop, report.timed_out
            ),
        }
    }
}

impl<StoreError: std::error::Error + 'static> std::error::Error for DriverError<StoreError> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Notification(error) => Some(error),
            Self::IncompleteNotification(_) => None,
        }
    }
}

pub(crate) fn execute<S: Store>(
    store: &mut S,
    command: Command,
    mut notify: impl FnMut() -> Result<Report, Failure>,
) -> Result<(), DriverError<S::Error>> {
    let change = match command {
        Command::Notify => PathChange::Changed,
        Command::Transaction(invocation) => {
            operation(store, &invocation).map_err(DriverError::Store)?
        }
    };
    if change == PathChange::Unchanged {
        return Ok(());
    }
    let report = notify().map_err(DriverError::Notification)?;
    if report.stop != Stop::Complete || report.timed_out != 0 {
        return Err(DriverError::IncompleteNotification(report));
    }
    Ok(())
}

fn operation<S: Store>(store: &mut S, invocation: &Invocation) -> Result<PathChange, S::Error> {
    if invocation.stage == Stage::Preflight {
        transaction::preflight(store, invocation.expected, &invocation.caller)?;
        return Ok(PathChange::Unchanged);
    }
    if invocation.stage == Stage::Prepare {
        prepare(store, invocation)?;
        return Ok(PathChange::Unchanged);
    }
    if invocation.stage == Stage::Rollback {
        return transaction::rollback_at(store, invocation.expected, &invocation.caller);
    }
    let id = invocation
        .expected
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    match invocation.stage {
        Stage::Apply => transaction::apply(store, id, &invocation.caller),
        Stage::Commit => {
            transaction::commit(store, id, &invocation.caller)?;
            Ok(PathChange::Unchanged)
        }
        Stage::Preflight | Stage::Prepare | Stage::Rollback => Err(Fault::WrongPhase.into()),
    }
}

fn prepare<S: Store>(store: &mut S, invocation: &Invocation) -> Result<(), S::Error> {
    let observed = store.load()?;
    if observed.state.sequence == invocation.expected {
        let request = invocation.caller.request(observed.state.owner.as_ref())?;
        transaction::begin_at(store, invocation.expected, &invocation.caller, request)?;
        return Ok(());
    }
    let wanted = invocation
        .expected
        .checked_add(1)
        .ok_or(Fault::SequenceExhausted)?;
    if observed.state.sequence != wanted {
        return Err(Fault::Conflict.into());
    }
    let record = observed
        .state
        .journal
        .as_ref()
        .filter(|record| record.id == wanted && record.caller == invocation.caller)
        .ok_or(Fault::WrongTransaction)?;
    if record.phase == crate::installer::state::Phase::RolledBack {
        return Err(Fault::WrongPhase.into());
    }
    Ok(())
}
