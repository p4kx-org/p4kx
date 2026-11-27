// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::discovery_test_support::{self, TestResult};

use super::ProtectedResult;

#[test]
fn a_protection_notice_preserves_the_primary_success_and_original_failure() -> TestResult {
    let measured =
        ProtectedResult::measured(discovery_test_support::issues()?, || Ok::<_, io::Error>(42));
    assert_eq!(
        measured.result?, 42,
        "technical protection notices do not replace successful data"
    );
    assert!(
        !measured.issues.is_empty(),
        "the original typed notices travel beside the result"
    );
    let failed = ProtectedResult::measured(discovery_test_support::issues()?, || {
        Err::<(), _>(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "primary archive sentinel",
        ))
    });
    let error = failed.result.err().ok_or("primary error")?;
    assert_eq!(
        error.kind(),
        io::ErrorKind::PermissionDenied,
        "the primary error kind is unchanged"
    );
    assert_eq!(
        error.to_string(),
        "primary archive sentinel",
        "the primary reason is unchanged"
    );
    assert!(
        !failed.issues.is_empty(),
        "a failed open still transports discovery diagnostics"
    );
    Ok(())
}

#[test]
fn comparison_pairing_keeps_both_reports_and_the_original_left_error_priority() -> TestResult {
    let left =
        ProtectedResult::measured(discovery_test_support::issues()?, || Err::<(), _>("left"));
    let right =
        ProtectedResult::measured(discovery_test_support::issues()?, || Err::<(), _>("right"));
    let count = left.issues.len() + right.issues.len();
    let paired = left.zip(right);
    assert_eq!(
        paired.result,
        Err("left"),
        "pairing retains the existing primary error priority"
    );
    assert_eq!(
        paired.issues.len(),
        count,
        "both source reports survive either archive failing"
    );
    Ok(())
}
