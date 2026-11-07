// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A failed source-bound verification without source values or identities.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct VerificationFailure;

pub(super) fn redact(result: super::TestResult) -> Result<(), VerificationFailure> {
    result.map_err(|_| VerificationFailure)
}

#[test]
fn source_and_schema_errors_never_enter_the_test_result() {
    let source = std::io::Error::other("PRIVATE_SOURCE_PATH_AND_RECORD_SENTINEL");
    let result = redact(Err(Box::new(source)));
    assert_eq!(
        result,
        Err(VerificationFailure),
        "source errors fail verification"
    );
    assert_eq!(
        format!("{result:?}"),
        "Err(VerificationFailure)",
        "only the static failure is printable"
    );
    assert_eq!(
        redact(Ok(())),
        Ok(()),
        "successful verification remains successful"
    );
}
