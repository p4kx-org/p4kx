// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_shader::{Budget, Limits, ReadError, SyntaxErrorKind};

use super::support::TestResult;

const UNSUPPORTED_DEPTH: usize = 129;
const SUPPORTED_DEPTH: usize = 128;

#[test]
fn a_configuration_refusal_precedes_invalid_utf8_and_has_no_source_range() -> TestResult {
    let error = p4kx_shader::read(
        &[0xff],
        Limits {
            depth: UNSUPPORTED_DEPTH,
            ..Limits::default()
        },
    )
    .err()
    .ok_or("unsupported configuration was accepted")?;
    assert_eq!(
        error.kind,
        SyntaxErrorKind::Limit {
            budget: Budget::Depth,
            limit: SUPPORTED_DEPTH
        }
    );
    assert_eq!(error.span.start, 0);
    assert_eq!(error.span.end_exclusive, 0);
    Ok(())
}

#[test]
fn source_size_and_the_original_checkpoint_error_precede_configuration_refusal() -> TestResult {
    let limits = Limits {
        source_bytes: 0,
        depth: UNSUPPORTED_DEPTH,
        ..Limits::default()
    };
    let error = p4kx_shader::read(&[0xff], limits)
        .err()
        .ok_or("the source byte bound was ignored")?;
    assert_eq!(
        error.kind,
        SyntaxErrorKind::Limit {
            budget: Budget::SourceBytes,
            limit: 0
        }
    );
    assert_eq!(error.span.start, 0);
    assert_eq!(error.span.end_exclusive, 1);
    let error = p4kx_shader::read_with(&[0xff], limits, || {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "original configuration checkpoint",
        ))
    })
    .err()
    .ok_or("the checkpoint refusal was ignored")?;
    let ReadError::Interrupted(cause) = error else {
        return Err("checkpoint error became a configuration failure".into());
    };
    assert_eq!(cause.kind(), io::ErrorKind::Interrupted);
    assert_eq!(cause.to_string(), "original configuration checkpoint");
    Ok(())
}

#[test]
fn pure_configuration_validation_agrees_with_the_original_reader_guard() -> TestResult {
    let invalid = Limits {
        depth: UNSUPPORTED_DEPTH,
        ..Limits::default()
    };
    let error = invalid
        .validate_configuration()
        .err()
        .ok_or("unsupported configuration was accepted")?;
    let read_error = p4kx_shader::read(b"", invalid)
        .err()
        .ok_or("the reader ignored invalid configuration")?;
    assert_eq!(
        error, read_error,
        "the format owns the exact canonical configuration diagnostic"
    );
    for depth in [0, SUPPORTED_DEPTH] {
        Limits {
            depth,
            ..Limits::default()
        }
        .validate_configuration()?;
    }
    Ok(())
}
