// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_msi_path::notification::{Failure, FailureKind, Report, Stop};

#[test]
fn native_codes_round_trip_without_zero_success_or_byte_truncation() {
    for (native, expected) in [
        (0, 0x2000_0000),
        (5, 0x2000_0005),
        (261, 0x2000_0105),
        (1460, 0x2000_05b4),
    ] {
        let actual = super::of(&Err(failure(FailureKind::Native(native))));
        assert_eq!(
            actual,
            Some(expected),
            "each representable native code is exact"
        );
        assert_ne!(actual, Some(0), "native zero is a real failure");
        assert_ne!(actual, Some(259), "no status aliases STILL_ACTIVE");
    }
}

#[test]
fn a_large_native_code_is_explicitly_not_representable() {
    assert_eq!(
        super::of(&Err(failure(FailureKind::Native(0x1fff_ffff)))),
        Some(0x3fff_ffff),
        "maximum payload retains every bit"
    );
    for native in [0x2000_0000, 0x6000_0001, u32::MAX] {
        assert_eq!(
            super::of(&Err(failure(FailureKind::Native(native)))),
            Some(0x6000_0004),
            "wide native codes are never truncated or mistaken for local classes"
        );
    }
}

#[test]
fn local_failures_have_distinct_non_native_statuses() {
    assert_eq!(
        super::of(&Err(failure(FailureKind::InvalidClock))),
        Some(0x6000_0001),
        "clock failure retains its category"
    );
    assert_eq!(
        super::of(&Err(failure(FailureKind::CurrentThread))),
        Some(0x6000_0002),
        "same-thread timeout rejection retains its category"
    );
}

#[test]
fn incomplete_status_preserves_stop_and_timeout_presence() {
    for (stop, timeouts, expected) in [
        (Stop::Complete, 1, 0x6000_0101),
        (Stop::Deadline, 0, 0x6000_0102),
        (Stop::Deadline, 1, 0x6000_0103),
        (Stop::WindowLimit, 0, 0x6000_0104),
        (Stop::WindowLimit, 1, 0x6000_0105),
    ] {
        assert_eq!(
            super::of(&Ok(report(stop, timeouts))),
            Some(expected),
            "stop and timeout occupy disjoint diagnostic slots"
        );
    }
}

#[test]
fn complete_notification_does_not_change_a_different_failure() {
    assert_eq!(
        super::of(&Ok(report(Stop::Complete, 0))),
        None,
        "complete delivery adds no notification fault"
    );
    assert_eq!(
        super::completed(false, None),
        1,
        "argument, registry and random failures retain generic failure"
    );
    assert_eq!(
        super::completed(true, None),
        0,
        "operation success remains success"
    );
    assert_eq!(
        super::completed(true, Some(0x6000_0001)),
        0,
        "diagnostics do not overrule the authoritative original result"
    );
    assert_eq!(
        super::completed(false, Some(0x2000_0005)),
        0x2000_0005,
        "a failed operation exposes its observed notification class"
    );
}

fn failure(kind: FailureKind) -> Failure {
    Failure {
        kind,
        progress: report(Stop::Complete, 0),
    }
}

fn report(stop: Stop, timed_out: u32) -> Report {
    Report {
        acknowledged: 0,
        timed_out,
        vanished: 0,
        excluded_own_process: 0,
        stop,
    }
}

#[test]
fn counted_helper_exclusions_do_not_become_an_unrelated_notification_fault() {
    let mut observed = report(Stop::Complete, 0);
    observed.excluded_own_process = 2;
    assert_eq!(
        super::of(&Ok(observed)),
        None,
        "a complete report retains own-process exclusions without changing status policy"
    );
}

#[test]
fn helper_exclusions_do_not_hide_an_external_timeout_status() {
    let mut observed = report(Stop::Complete, 1);
    observed.excluded_own_process = 1;
    assert_eq!(
        super::of(&Ok(observed)),
        Some(0x6000_0101),
        "the unchanged status mapper still exposes the genuine external timeout"
    );
}
