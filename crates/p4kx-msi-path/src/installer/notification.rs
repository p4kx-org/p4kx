// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Safe transport contract for the fixed Windows environment notification.

use std::time::{Duration, Instant};

/// The immutable NUL-terminated UTF16 `WM_SETTINGCHANGE` payload.
pub const ENVIRONMENT: [u16; 12] = [69, 110, 118, 105, 114, 111, 110, 109, 101, 110, 116, 0];
/// The maximum number of top-level recipients captured by one query.
pub const WINDOW_LIMIT: usize = 1024;
/// The shared wait budget for enumeration and recipient delivery.
pub const TOTAL_BUDGET: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Why enumeration or recipient delivery stopped.
pub enum Stop {
    /// All observed recipients were processed.
    Complete,
    /// The shared deadline expired.
    Deadline,
    /// Additional recipients exceed the bounded inventory.
    WindowLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// The result of one synchronous native delivery.
pub enum Delivery {
    /// The target completed the system-message call.
    Acknowledged,
    /// The native API explicitly reported a timeout.
    TimedOut,
    /// The native API explicitly reported a destroyed window.
    WindowGone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Actual delivery counts and any incomplete inventory boundary.
pub struct Report {
    /// Completed native recipient calls.
    pub acknowledged: u32,
    /// Calls explicitly classified as timed out.
    pub timed_out: u32,
    /// Recipients explicitly classified as destroyed.
    pub vanished: u32,
    /// The operation's completion boundary.
    pub stop: Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A typed failure retaining progress without inventing delivery outcomes.
pub struct Failure {
    /// The original native code or local contract failure.
    pub kind: FailureKind,
    /// Deliveries completed before failure.
    pub progress: Report,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "environment notification failed: {:?}",
            self.kind
        )
    }
}

impl std::error::Error for Failure {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// A failure at the clock or native transport boundary.
pub enum FailureKind {
    /// The native code, including zero when no detail was available.
    Native(u32),
    /// The clock cannot represent the bounded deadline.
    InvalidClock,
    /// A same-thread recipient would ignore the native timeout.
    CurrentThread,
}

/// One bounded batch of actual native recipient handles.
pub struct Enumeration<W> {
    /// Recipients captured before the enumeration boundary.
    pub windows: Vec<W>,
    /// Whether enumeration completed or reached a bound.
    pub stop: Stop,
}

/// Supplies a monotonic clock sharing the transport's deadline domain.
pub trait Clock {
    /// Returns the current monotonic instant.
    fn now(&self) -> Instant;
}

/// Performs bounded recipient enumeration and fixed-message delivery.
pub trait Transport {
    /// An observed native window identity.
    type Window: Copy;
    /// Enumerates at most `maximum` recipients before `deadline`.
    ///
    /// # Errors
    /// Returns the native failure without replacing its code.
    fn enumerate(
        &mut self,
        deadline: Instant,
        maximum: usize,
    ) -> Result<Enumeration<Self::Window>, FailureKind>;
    /// Sends the fixed Environment system message using the remaining wait budget.
    ///
    /// # Errors
    /// Returns the native failure or a recipient that cannot be safely bounded.
    fn send(&mut self, window: Self::Window, timeout_ms: u32) -> Result<Delivery, FailureKind>;
}

/// Classifies one native thread lookup without discarding its immediate error.
///
/// # Errors
/// Returns every failed lookup except the supplied native invalid-window code.
pub const fn thread_lookup(
    thread: u32,
    error: u32,
    invalid_handle: u32,
) -> Result<Option<u32>, FailureKind> {
    if thread != 0 {
        return Ok(Some(thread));
    }
    if error == invalid_handle {
        Ok(None)
    } else {
        Err(FailureKind::Native(error))
    }
}

/// Shares one deadline between enumeration and every per-window synchronous send.
///
/// # Errors
/// Returns a native transport failure or an unrepresentable clock deadline.
pub fn notify(transport: &mut impl Transport, clock: &impl Clock) -> Result<Report, Failure> {
    let mut report = Report {
        acknowledged: 0,
        timed_out: 0,
        vanished: 0,
        stop: Stop::Complete,
    };
    let deadline = clock
        .now()
        .checked_add(TOTAL_BUDGET)
        .ok_or_else(|| Failure {
            kind: FailureKind::InvalidClock,
            progress: report.clone(),
        })?;
    let enumeration = transport
        .enumerate(deadline, WINDOW_LIMIT)
        .map_err(|kind| Failure {
            kind,
            progress: report.clone(),
        })?;
    report.stop = enumeration.stop;
    for window in enumeration.windows {
        let remaining = deadline.saturating_duration_since(clock.now());
        let timeout_ms = u32::try_from(remaining.as_millis()).map_err(|_| Failure {
            kind: FailureKind::InvalidClock,
            progress: report.clone(),
        })?;
        if timeout_ms == 0 {
            report.stop = Stop::Deadline;
            break;
        }
        let delivery = transport.send(window, timeout_ms).map_err(|kind| Failure {
            kind,
            progress: report.clone(),
        })?;
        match delivery {
            Delivery::Acknowledged => report.acknowledged += 1,
            Delivery::TimedOut => report.timed_out += 1,
            Delivery::WindowGone => report.vanished += 1,
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests;
