// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Encodes the observed notification result in a private Windows process status.

use p4kx_msi_path::notification::{Failure, FailureKind, Report, Stop};

const GENERIC_FAILURE: i32 = 1;
const NATIVE_TAG: i32 = 0x2000_0000;
const NATIVE_LIMIT: i32 = 0x1fff_ffff;
const INVALID_CLOCK: i32 = 0x6000_0001;
const CURRENT_THREAD: i32 = 0x6000_0002;
const UNKNOWN_FAILURE: i32 = 0x6000_0003;
const NATIVE_OUT_OF_RANGE: i32 = 0x6000_0004;
const INCOMPLETE_BASE: i32 = 0x6000_0100;

/// Returns a diagnostic status without changing the operation's result.
#[must_use]
pub(crate) fn of(result: &Result<Report, Failure>) -> Option<i32> {
    match result {
        Err(failure) => Some(match failure.kind {
            FailureKind::Native(code) => native(code),
            FailureKind::InvalidClock => INVALID_CLOCK,
            FailureKind::CurrentThread => CURRENT_THREAD,
            _ => UNKNOWN_FAILURE,
        }),
        Ok(report) => incomplete(report.stop, report.timed_out),
    }
}

/// Preserves success and failure as decided by the original operation.
#[must_use]
pub(crate) const fn completed(successful: bool, observed: Option<i32>) -> i32 {
    if successful {
        0
    } else {
        match observed {
            Some(status) => status,
            None => GENERIC_FAILURE,
        }
    }
}

fn native(code: u32) -> i32 {
    match i32::try_from(code) {
        Ok(payload @ 0..=NATIVE_LIMIT) => NATIVE_TAG | payload,
        _ => NATIVE_OUT_OF_RANGE,
    }
}

const fn incomplete(stop: Stop, timed_out: u32) -> Option<i32> {
    let timeout = if timed_out == 0 { 0 } else { 1 };
    let offset = match stop {
        Stop::Complete if timed_out == 0 => return None,
        Stop::Complete => 0,
        Stop::Deadline => 2,
        Stop::WindowLimit => 4,
        _ => 6,
    };
    Some(INCOMPLETE_BASE + offset + timeout)
}

#[cfg(test)]
#[path = "notification_status_tests.rs"]
mod tests;
