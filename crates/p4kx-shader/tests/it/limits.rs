// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_shader::{Budget, Limits, ReadError, SyntaxErrorKind};

use super::support::TestResult;

fn requires_limit(source: &[u8], limits: Limits, budget: Budget, bound: usize) -> TestResult {
    let error = p4kx_shader::read(source, limits)
        .err()
        .ok_or("source exceeded its bound without refusal")?;
    assert_eq!(
        error.kind,
        SyntaxErrorKind::Limit {
            budget,
            limit: bound
        },
        "the responsible bound is explicit"
    );
    assert!(
        error.span.end_exclusive <= source.len(),
        "limit diagnostics remain source-local"
    );
    Ok(())
}

#[test]
fn source_bytes_accept_the_exact_bound_and_reject_the_next_byte() -> TestResult {
    p4kx_shader::read(
        b"A",
        Limits {
            source_bytes: 1,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"AB",
        Limits {
            source_bytes: 1,
            ..Limits::default()
        },
        Budget::SourceBytes,
        1,
    )?;
    Ok(())
}

#[test]
fn atom_and_quote_bytes_accept_exact_bounds_and_reject_the_next_byte() -> TestResult {
    p4kx_shader::read(
        b"AB",
        Limits {
            token_bytes: 2,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"ABC",
        Limits {
            token_bytes: 2,
            ..Limits::default()
        },
        Budget::TokenBytes,
        2,
    )?;
    p4kx_shader::read(
        b"A('x')",
        Limits {
            token_bytes: 3,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"A('xy')",
        Limits {
            token_bytes: 3,
            ..Limits::default()
        },
        Budget::TokenBytes,
        3,
    )?;
    Ok(())
}

#[test]
fn comment_and_value_bytes_accept_exact_bounds_and_reject_the_next_byte() -> TestResult {
    p4kx_shader::read(
        b"/*x*/",
        Limits {
            token_bytes: 5,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"/*xy*/",
        Limits {
            token_bytes: 5,
            ..Limits::default()
        },
        Budget::TokenBytes,
        5,
    )?;
    p4kx_shader::read(
        b"A(ab)",
        Limits {
            value_bytes: 2,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"A(abc)",
        Limits {
            value_bytes: 2,
            ..Limits::default()
        },
        Budget::ValueBytes,
        2,
    )?;
    requires_limit(
        b"A =  x ",
        Limits {
            value_bytes: 3,
            ..Limits::default()
        },
        Budget::ValueBytes,
        3,
    )?;
    Ok(())
}

#[test]
fn count_bounds_include_duplicate_and_unknown_scope_work() -> TestResult {
    p4kx_shader::read(
        b"A(B)",
        Limits {
            tokens: 4,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"A(B)",
        Limits {
            tokens: 3,
            ..Limits::default()
        },
        Budget::Tokens,
        3,
    )?;
    p4kx_shader::read(
        b"A;B",
        Limits {
            declarations: 2,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"A;B",
        Limits {
            declarations: 1,
            ..Limits::default()
        },
        Budget::Declarations,
        1,
    )?;
    let source = b"Property{} Unknown{Property{}}";
    p4kx_shader::read(
        source,
        Limits {
            properties: 2,
            ..Limits::default()
        },
    )?;
    requires_limit(
        source,
        Limits {
            properties: 1,
            ..Limits::default()
        },
        Budget::Properties,
        1,
    )?;
    Ok(())
}

#[test]
fn combined_nesting_has_exact_and_hard_bounds() -> TestResult {
    let source = b"A{B((x))}";
    p4kx_shader::read(
        source,
        Limits {
            depth: 3,
            ..Limits::default()
        },
    )?;
    requires_limit(
        source,
        Limits {
            depth: 2,
            ..Limits::default()
        },
        Budget::Depth,
        2,
    )?;
    requires_limit(
        b"",
        Limits {
            depth: 129,
            ..Limits::default()
        },
        Budget::Depth,
        128,
    )?;
    p4kx_shader::read(
        b"A",
        Limits {
            depth: 0,
            ..Limits::default()
        },
    )?;
    requires_limit(
        b"A()",
        Limits {
            depth: 0,
            ..Limits::default()
        },
        Budget::Depth,
        0,
    )?;
    Ok(())
}

#[test]
fn checkpoint_errors_remain_typed_across_every_stage() -> TestResult {
    for stop in 1..=6 {
        let mut calls = 0;
        let result = p4kx_shader::read_with(b"A", Limits::default(), || {
            calls += 1;
            if calls == stop {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "synthetic checkpoint marker",
                ))
            } else {
                Ok(())
            }
        });
        let error = result.err().ok_or("checkpoint refusal was ignored")?;
        if let ReadError::Interrupted(error) = error {
            assert_eq!(
                error.kind(),
                io::ErrorKind::Interrupted,
                "caller error type and category survive"
            );
            assert_eq!(
                error.to_string(),
                "synthetic checkpoint marker",
                "caller error detail survives unchanged"
            );
        } else {
            return Err("checkpoint failure became a syntax error".into());
        }
        assert_eq!(
            calls, stop,
            "parser stops immediately after checkpoint refusal"
        );
    }
    Ok(())
}

#[test]
fn long_comments_poll_cancellation_instead_of_waiting_for_their_end() -> TestResult {
    let source = format!("/*{}*/", "x".repeat(4096));
    let mut calls = 0;
    let error = p4kx_shader::read_with(source.as_bytes(), Limits::default(), || {
        calls += 1;
        if calls == 500 {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "comment cancelled",
            ))
        } else {
            Ok(())
        }
    })
    .err()
    .ok_or("long comment ignored cancellation")?;
    assert!(
        matches!(error, ReadError::Interrupted(_)),
        "cancellation cause remains distinct from malformed comment"
    );
    assert_eq!(calls, 500, "no additional callback runs after cancellation");
    Ok(())
}
