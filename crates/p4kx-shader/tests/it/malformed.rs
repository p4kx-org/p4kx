// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Construct, Limits, SyntaxErrorKind};

use super::support::TestResult;

#[test]
fn unterminated_comments_quotes_and_groups_report_original_offsets() -> TestResult {
    let cases: &[(&[u8], Construct, usize)] = &[
        (b"x\n/* unfinished", Construct::Comment, 2),
        (b"A(\"escaped\\\"", Construct::Quote, 2),
        (b"Property { Name(x)", Construct::Brace, 9),
        (b"A(value", Construct::Parenthesis, 1),
        (b"A = (value", Construct::Parenthesis, 4),
    ];
    for &(source, construct, start) in cases {
        let error = p4kx_shader::read(source, Limits::default())
            .err()
            .ok_or("unterminated source was accepted")?;
        assert_eq!(
            error.kind,
            SyntaxErrorKind::Unterminated(construct),
            "closing delimiter must not be guessed"
        );
        assert_eq!(
            (error.span.start, error.span.end_exclusive),
            (start, source.len()),
            "error offsets refer to original bytes"
        );
    }
    Ok(())
}

#[test]
fn mismatched_or_unowned_closing_delimiters_are_not_silently_skipped() -> TestResult {
    let cases: &[(&[u8], Construct)] = &[
        (b"}", Construct::Brace),
        (b")", Construct::Parenthesis),
        (b"A{ ) }", Construct::Parenthesis),
        (b"A( } )", Construct::Brace),
        (b"A = x)", Construct::Parenthesis),
    ];
    for &(source, construct) in cases {
        let error = p4kx_shader::read(source, Limits::default())
            .err()
            .ok_or("mismatched close was accepted")?;
        assert_eq!(
            error.kind,
            SyntaxErrorKind::UnexpectedClosing(construct),
            "unmatched closing punctuation is structural failure"
        );
        assert_eq!(
            error.span.end_exclusive - error.span.start,
            1,
            "the offending ASCII delimiter has a precise range"
        );
    }
    Ok(())
}

#[test]
fn invalid_encoding_is_explicit_and_never_decoded_lossily() -> TestResult {
    for source in [
        b"Name(\xff)".as_slice(),
        b"\xff\xfeA\0".as_slice(),
        b"A\xe2\x82".as_slice(),
    ] {
        let error = p4kx_shader::read(source, Limits::default())
            .err()
            .ok_or("invalid encoding was accepted")?;
        assert_eq!(
            error.kind,
            SyntaxErrorKind::UnknownEncoding,
            "unsupported bytes do not become replacement characters"
        );
        assert!(
            error.span.start < source.len(),
            "encoding diagnostic points into its source"
        );
        assert!(
            error.span.end_exclusive <= source.len(),
            "encoding range remains within source bounds"
        );
    }
    Ok(())
}
