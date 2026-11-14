// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::notification;
use std::cell::Cell;
use std::time::{Duration, Instant};

use super::{Clock, Delivery, Enumeration, FailureKind, Stop, Transport};

struct FakeClock<'a> {
    start: Instant,
    elapsed: &'a Cell<u64>,
}
impl Clock for FakeClock<'_> {
    fn now(&self) -> Instant {
        self.start + Duration::from_millis(self.elapsed.get())
    }
}

struct FakeTransport<'a> {
    elapsed: &'a Cell<u64>,
    enumeration_cost: u64,
    stop: Stop,
    windows: usize,
    failure: Option<FailureKind>,
    own_process: Option<usize>,
    timeouts: Vec<u32>,
}

impl Transport for FakeTransport<'_> {
    type Window = usize;
    fn enumerate(
        &mut self,
        _deadline: Instant,
        maximum: usize,
    ) -> Result<Enumeration<usize>, FailureKind> {
        assert_eq!(
            maximum,
            notification::WINDOW_LIMIT,
            "enumeration has a fixed work bound"
        );
        self.elapsed.set(self.elapsed.get() + self.enumeration_cost);
        Ok(Enumeration {
            windows: (0..self.windows).collect(),
            stop: self.stop,
        })
    }
    fn send(&mut self, window: usize, timeout_ms: u32) -> Result<Delivery, FailureKind> {
        self.timeouts.push(timeout_ms);
        if self.own_process == Some(window) {
            return Ok(Delivery::OwnProcess);
        }
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.elapsed
            .set(self.elapsed.get() + u64::from(timeout_ms.min(600)));
        match window {
            0 => Ok(Delivery::Acknowledged),
            1 => Ok(Delivery::TimedOut),
            _ => Ok(Delivery::WindowGone),
        }
    }
}

fn transport(elapsed: &Cell<u64>) -> FakeTransport<'_> {
    FakeTransport {
        elapsed,
        enumeration_cost: 0,
        stop: Stop::Complete,
        windows: 9,
        failure: None,
        own_process: None,
        timeouts: Vec::new(),
    }
}

#[test]
fn all_windows_share_one_deadline_instead_of_each_receiving_a_full_timeout() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    let report = notification::notify(&mut transport, &clock);
    assert_eq!(
        transport.timeouts,
        [1500, 900, 300],
        "remaining timeout shrinks across recipients"
    );
    assert_eq!(
        elapsed.get(),
        1500,
        "all simulated sends share the total budget"
    );
    assert!(
        matches!(report, Ok(ref value) if value.stop == Stop::Deadline && value.acknowledged == 1 && value.timed_out == 1 && value.vanished == 1),
        "partial delivery remains explicit"
    );
}

#[test]
fn enumeration_exhaustion_sends_nothing_and_retains_the_limit() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.enumeration_cost = 1500;
    transport.stop = Stop::WindowLimit;
    let report = notification::notify(&mut transport, &clock);
    assert!(
        transport.timeouts.is_empty(),
        "enumeration time consumes the same total budget"
    );
    assert!(
        matches!(report, Ok(ref value) if value.stop == Stop::Deadline),
        "deadline is not reported as complete notification"
    );
}

#[test]
fn native_failure_is_preserved_without_fabricating_successful_recipients() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.failure = Some(FailureKind::Native(5));
    let report = notification::notify(&mut transport, &clock);
    assert!(
        matches!(report, Err(ref error) if error.kind == FailureKind::Native(5) && error.progress.acknowledged == 0),
        "original native code is available to the caller"
    );
}

#[test]
fn the_payload_is_exact_static_nul_terminated_environment_utf16() {
    let expected: Vec<u16> = "Environment"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    assert_eq!(
        notification::ENVIRONMENT.as_slice(),
        expected,
        "no stack string pointer enters a timed-out native message"
    );
}

#[test]
fn zero_native_error_is_not_invented_as_a_timeout() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.failure = Some(FailureKind::Native(0));
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Err(ref error) if error.kind == FailureKind::Native(0) && error.progress.timed_out == 0),
        "unclassified native failure stays distinct from timeout"
    );
}

#[test]
fn current_thread_rejection_cannot_enter_an_unbounded_same_thread_send() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.failure = Some(FailureKind::CurrentThread);
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Err(ref error) if error.kind == FailureKind::CurrentThread),
        "queue-safety rejection remains a typed failure"
    );
    assert_eq!(
        elapsed.get(),
        0,
        "rejected recipient consumes no synchronous wait"
    );
}

#[test]
fn reaching_the_window_count_limit_is_reported_after_the_bounded_list() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.windows = 2;
    transport.stop = Stop::WindowLimit;
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Ok(ref report) if report.stop == Stop::WindowLimit && report.acknowledged == 1 && report.timed_out == 1),
        "unvisited windows cannot become a complete report"
    );
}

#[test]
fn invalid_handle_lookup_is_the_only_disappeared_window_case() {
    const INVALID_WINDOW_HANDLE: u32 = 1400;
    assert_eq!(
        notification::thread_lookup(0, INVALID_WINDOW_HANDLE, INVALID_WINDOW_HANDLE),
        Ok(None),
        "native invalid HWND is an actual vanished window"
    );
    assert_eq!(
        notification::thread_lookup(42, INVALID_WINDOW_HANDLE, INVALID_WINDOW_HANDLE),
        Ok(Some(42)),
        "a successful lookup keeps its actual thread identity"
    );
}

#[test]
fn zero_thread_lookup_with_another_native_error_retains_the_code() {
    const INVALID_WINDOW_HANDLE: u32 = 1400;
    assert_eq!(
        notification::thread_lookup(0, 5, INVALID_WINDOW_HANDLE),
        Err(FailureKind::Native(5)),
        "access or system failures cannot become a vanished window"
    );
}

#[test]
fn zero_thread_lookup_without_a_last_error_remains_a_generic_failure() {
    const INVALID_WINDOW_HANDLE: u32 = 1400;
    assert_eq!(
        notification::thread_lookup(0, 0, INVALID_WINDOW_HANDLE),
        Err(FailureKind::Native(0)),
        "missing native error detail is preserved without invented meaning"
    );
}

#[test]
fn helper_windows_are_counted_separately_without_claiming_delivery() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.windows = 2;
    transport.own_process = Some(1);
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Ok(ref report) if report.stop == Stop::Complete && report.acknowledged == 1 && report.excluded_own_process == 1 && report.vanished == 0 && report.timed_out == 0),
        "the helper exclusion is neither an acknowledgement nor a disappeared window"
    );
    assert_eq!(
        transport.timeouts,
        [1500, 900],
        "own-recipient classification does not reset the shared deadline"
    );
    assert_eq!(elapsed.get(), 600, "only the foreign simulated send waits");
}

#[test]
fn excluding_a_helper_does_not_swallow_foreign_native_or_thread_failures() {
    for kind in [
        FailureKind::Native(0),
        FailureKind::Native(5),
        FailureKind::CurrentThread,
    ] {
        let elapsed = Cell::new(0);
        let clock = FakeClock {
            start: Instant::now(),
            elapsed: &elapsed,
        };
        let mut transport = transport(&elapsed);
        transport.windows = 2;
        transport.own_process = Some(0);
        transport.failure = Some(kind);
        let result = notification::notify(&mut transport, &clock);
        assert!(
            matches!(result, Err(ref error) if error.kind == kind && error.progress.excluded_own_process == 1 && error.progress.acknowledged == 0 && error.progress.timed_out == 0 && error.progress.vanished == 0),
            "an excluded helper does not turn a later foreign failure into successful notification"
        );
        assert_eq!(
            transport.timeouts.len(),
            2,
            "the actual foreign recipient is still inspected"
        );
    }
}

#[test]
fn exhausted_enumeration_does_not_invent_observed_helper_exclusions() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.windows = 1;
    transport.own_process = Some(0);
    transport.enumeration_cost = 1500;
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Ok(ref report) if report.stop == Stop::Deadline && report.excluded_own_process == 0),
        "uninspected recipient identities cannot be stamped as excluded"
    );
    assert!(
        transport.timeouts.is_empty(),
        "the deadline still stops every per-window operation"
    );
}

#[test]
fn observed_helper_exclusions_do_not_hide_the_enumeration_count_limit() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.windows = 1;
    transport.own_process = Some(0);
    transport.stop = Stop::WindowLimit;
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Ok(ref report) if report.stop == Stop::WindowLimit && report.excluded_own_process == 1 && report.acknowledged == 0),
        "excluded observations do not establish that unvisited windows are all helper-owned"
    );
}

#[test]
fn a_foreign_timeout_stays_separate_from_an_excluded_helper() {
    let elapsed = Cell::new(0);
    let clock = FakeClock {
        start: Instant::now(),
        elapsed: &elapsed,
    };
    let mut transport = transport(&elapsed);
    transport.windows = 2;
    transport.own_process = Some(0);
    let result = notification::notify(&mut transport, &clock);
    assert!(
        matches!(result, Ok(ref report) if report.stop == Stop::Complete && report.excluded_own_process == 1 && report.timed_out == 1 && report.acknowledged == 0 && report.vanished == 0),
        "own-process exclusion cannot turn an external timeout into successful delivery"
    );
}
