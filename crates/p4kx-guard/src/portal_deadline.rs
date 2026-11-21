// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};

const CANCELLATION_POLL: Duration = Duration::from_millis(10);

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Finished,
    TimedOut,
    Cancelled,
}

pub(crate) fn with_socket_deadline<Output>(
    deadline: Instant,
    cancel: &AtomicBool,
    shutdown: impl FnOnce() -> io::Result<()> + Send,
    work: impl FnOnce() -> Result<Output, UnknownAlias>,
) -> Result<Output, UnknownAlias> {
    thread::scope(|scope| {
        let (finished, receive) = mpsc::channel::<()>();
        let supervisor = thread::Builder::new()
            .name("portal-deadline".to_owned())
            .spawn_scoped(scope, move || {
                let stop = loop {
                    if cancel.load(Ordering::Acquire) {
                        break Stop::Cancelled;
                    }
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break Stop::TimedOut;
                    }
                    match receive.recv_timeout(remaining.min(CANCELLATION_POLL)) {
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break Stop::Finished,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                };
                if stop != Stop::Finished {
                    match shutdown() {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotConnected => {}
                        Err(error) => return Err(error),
                    }
                }
                Ok(stop)
            })
            .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))?;
        let result = work();
        drop(finished);
        let stopped = supervisor
            .join()
            .map_err(|_| {
                UnknownAlias::with_source(
                    AliasUncertainty::SessionBus,
                    io::Error::other("portal deadline supervisor failed"),
                )
            })?
            .map_err(|error| UnknownAlias::with_source(AliasUncertainty::SessionBus, error))?;
        if stopped == Stop::Cancelled || cancel.load(Ordering::Acquire) {
            return Err(UnknownAlias::new(AliasUncertainty::Cancelled));
        }
        if stopped == Stop::TimedOut || Instant::now() >= deadline {
            return Err(UnknownAlias::new(AliasUncertainty::Timeout));
        }
        result
    })
}
