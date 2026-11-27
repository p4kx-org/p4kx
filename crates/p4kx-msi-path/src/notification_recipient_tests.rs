// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_msi_path::notification::FailureKind;

use super::{Owner, Recipient};

const HELPER: Owner = Owner {
    thread: 7,
    process: 31,
};

#[test]
fn a_proven_helper_process_window_is_excluded_on_either_thread() {
    for thread in [HELPER.thread, 8] {
        assert_eq!(
            super::classify(
                Owner {
                    thread,
                    process: HELPER.process
                },
                HELPER
            ),
            Ok(Recipient::OwnProcess),
            "only the observed helper PID authorizes exclusion, including another helper thread"
        );
    }
}

#[test]
fn a_foreign_process_window_remains_an_external_recipient() {
    assert_eq!(
        super::classify(
            Owner {
                thread: 8,
                process: 32
            },
            HELPER
        ),
        Ok(Recipient::External),
        "another process is not covered by the helper exclusion"
    );
}

#[test]
fn unknown_or_foreign_ownership_on_the_current_thread_stays_fail_closed() {
    for process in [0, 32] {
        assert_eq!(
            super::classify(
                Owner {
                    thread: HELPER.thread,
                    process
                },
                HELPER
            ),
            Err(FailureKind::CurrentThread),
            "thread equality does not manufacture helper-process ownership"
        );
    }
}

#[test]
fn a_missing_helper_process_identity_never_authorizes_exclusion() {
    assert_eq!(
        super::classify(
            Owner {
                thread: 7,
                process: 0
            },
            Owner {
                thread: 7,
                process: 0
            }
        ),
        Err(FailureKind::CurrentThread),
        "matching zero PIDs are not ownership evidence"
    );
    assert_eq!(
        super::classify(
            Owner {
                thread: 8,
                process: 0
            },
            HELPER
        ),
        Ok(Recipient::External),
        "an unknown PID is not an excluded helper recipient"
    );
}

#[test]
fn a_zero_thread_never_becomes_a_proven_helper_window() {
    assert_ne!(
        super::classify(
            Owner {
                thread: 0,
                process: HELPER.process
            },
            HELPER
        ),
        Ok(Recipient::OwnProcess),
        "a stale PID output cannot bypass the required successful thread lookup"
    );
}
