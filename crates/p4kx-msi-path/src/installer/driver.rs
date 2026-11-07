// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::installer::arguments::Command;
use crate::installer::bound_phase;
use crate::installer::invocation_nonce::{InvocationNonce, NonceError};
use crate::installer::notification::{Failure, Report, Stop};
use crate::installer::reservation;
use crate::installer::state::{PathChange, Store};

#[derive(Debug)]
pub(crate) enum DriverError<StoreError> {
    Store(StoreError),
    Random(NonceError),
    Notification(Failure),
    IncompleteNotification(Report),
}

impl<StoreError: fmt::Display> fmt::Display for DriverError<StoreError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => fmt::Display::fmt(error, formatter),
            Self::Random(error) => fmt::Display::fmt(error, formatter),
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
            Self::Random(error) => Some(error),
            Self::Notification(error) => Some(error),
            Self::IncompleteNotification(_) => None,
        }
    }
}

pub(crate) fn execute<S: Store>(
    store: &mut S,
    command: Command,
    mut nonce: impl FnMut() -> Result<InvocationNonce, NonceError>,
    mut notify: impl FnMut() -> Result<Report, Failure>,
) -> Result<(), DriverError<S::Error>> {
    let change = match command {
        Command::Notify => PathChange::Changed,
        Command::Transaction(invocation) => {
            bound_phase::execute(store, invocation).map_err(DriverError::Store)?
        }
        Command::Reserve(declaration) => {
            let caller = declaration
                .bind(nonce().map_err(DriverError::Random)?)
                .map_err(|error| DriverError::Store(error.into()))?;
            reservation::create(store, caller).map_err(DriverError::Store)?;
            PathChange::Unchanged
        }
        Command::Recover(binding) => {
            reservation::recover(store, binding, nonce().map_err(DriverError::Random)?)
                .map_err(DriverError::Store)?;
            PathChange::Unchanged
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
