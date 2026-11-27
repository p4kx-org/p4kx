// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use super::{ProbeOutput, check};

fn output(success: bool, stdout: &[u8]) -> ProbeOutput {
    ProbeOutput {
        success,
        stdout: stdout.to_vec(),
        stderr: b"command failed".to_vec(),
    }
}

fn failure(result: io::Result<bool>) -> io::Result<io::Error> {
    result
        .err()
        .ok_or_else(|| io::Error::other("expected an unknown process status"))
}

#[test]
fn qprocess_success_does_not_launch_the_fallback() -> io::Result<()> {
    for (text, expected) in [
        (b"user console 1 4242 StarCitizen.exe\r\n".as_slice(), true),
        (b"HEADER\r\n".as_slice(), false),
    ] {
        let result = check(Ok(output(true, text)), || {
            Err(io::Error::other("unexpected fallback"))
        })?;
        assert_eq!(result, expected, "a successful query is conclusive");
    }
    Ok(())
}

#[test]
fn missing_qprocess_uses_a_valid_count() -> io::Result<()> {
    for (text, expected) in [
        (b"0\r\n".as_slice(), false),
        (b"1\r\n".as_slice(), true),
        (b"123".as_slice(), true),
    ] {
        let missing = io::Error::new(io::ErrorKind::NotFound, "qprocess absent");
        assert_eq!(
            check(Err(missing), || Ok(output(true, text)))?,
            expected,
            "a valid fallback count establishes the process status"
        );
    }
    Ok(())
}

#[test]
fn unsuccessful_qprocess_including_no_match_requires_the_fallback() -> io::Result<()> {
    for text in [
        b"No Process exists for StarCitizen.exe".as_slice(),
        b"query failed".as_slice(),
    ] {
        assert!(
            !check(Ok(output(false, text)), || Ok(output(true, b"0")))?,
            "a successful zero fallback establishes absence"
        );
        assert!(
            check(Ok(output(false, text)), || Ok(output(true, b"1")))?,
            "a successful positive fallback establishes presence"
        );
    }
    Ok(())
}

#[test]
fn qprocess_io_failure_can_be_resolved_by_the_fallback() -> io::Result<()> {
    let denied = io::Error::new(io::ErrorKind::PermissionDenied, "qprocess denied");
    assert!(
        !check(Err(denied), || Ok(output(true, b"0")))?,
        "a failed first command does not override a conclusive fallback"
    );
    Ok(())
}

#[test]
fn both_missing_commands_preserve_the_unknown_reason() -> io::Result<()> {
    let error = failure(check(
        Err(io::Error::new(io::ErrorKind::NotFound, "qprocess absent")),
        || Err(io::Error::new(io::ErrorKind::NotFound, "PowerShell absent")),
    ))?;
    assert_eq!(
        error.kind(),
        io::ErrorKind::NotFound,
        "the final I/O reason is typed"
    );
    assert!(
        error.to_string().contains("qprocess absent"),
        "the first failure is retained"
    );
    assert!(
        error.to_string().contains("PowerShell absent"),
        "the second failure is retained"
    );
    Ok(())
}

#[test]
fn an_unsuccessful_fallback_is_unknown_even_with_a_zero_count() -> io::Result<()> {
    let error = failure(check(Ok(output(false, b"")), || Ok(output(false, b"0"))))?;
    assert_eq!(
        error.kind(),
        io::ErrorKind::Other,
        "a failed command is not absence"
    );
    assert!(
        error.to_string().contains("command failed"),
        "the command error is retained"
    );
    Ok(())
}

#[test]
fn a_fallback_io_error_is_unknown() -> io::Result<()> {
    let error = failure(check(Ok(output(false, b"")), || {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
    }))?;
    assert_eq!(
        error.kind(),
        io::ErrorKind::PermissionDenied,
        "the fallback cannot establish absence"
    );
    Ok(())
}

#[test]
fn malformed_and_overflowing_counts_are_unknown() -> io::Result<()> {
    for text in [
        b"".as_slice(),
        b"error".as_slice(),
        b"-1".as_slice(),
        b"4294967296".as_slice(),
        b"1\n0".as_slice(),
        &[0xff],
    ] {
        let error = failure(check(Ok(output(false, b"")), || Ok(output(true, text))))?;
        assert_eq!(
            error.kind(),
            io::ErrorKind::InvalidData,
            "malformed counts are not zero"
        );
    }
    Ok(())
}
