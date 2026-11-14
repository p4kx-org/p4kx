// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{ShaderBudget, ShaderConstruct, ShaderDefinitionFailure, ShaderFailureReason};
use p4kx_shader::{Budget, Construct, Limits, SyntaxErrorKind};

use super::super::TestResult;
use super::super::shader_accounting::require;
use super::{ExpectedSource, check_span};

pub(super) fn check_failure(
    actual: &ShaderDefinitionFailure,
    expected: &ExpectedSource,
    bytes: &[u8],
) -> TestResult {
    let discovered = expected
        .failure
        .ok_or("readable source was exported as failed")?;
    let replay = p4kx_shader::read(bytes, Limits::default())
        .err()
        .ok_or("retained bytes do not reproduce the original read failure")?;
    require(
        discovered == replay,
        "retained bytes reproduce exact original diagnostic",
    )?;
    check_span(actual.span, replay.span.start, replay.span.end_exclusive)?;
    let reason = expected_reason(replay.kind)?;
    require(
        actual.failure == reason,
        "the structured failure reason differs",
    )
}

fn expected_reason(kind: SyntaxErrorKind) -> TestResult<ShaderFailureReason> {
    match kind {
        SyntaxErrorKind::UnknownEncoding => Ok(ShaderFailureReason::UnknownEncoding),
        SyntaxErrorKind::Allocation => Ok(ShaderFailureReason::Allocation),
        SyntaxErrorKind::Limit { budget, limit } => Ok(ShaderFailureReason::Limit {
            budget: native_budget(budget)?,
            limit: u64::try_from(limit)?,
        }),
        SyntaxErrorKind::Unterminated(kind) => Ok(ShaderFailureReason::Unterminated {
            construct: construct(kind)?,
        }),
        SyntaxErrorKind::UnexpectedClosing(kind) => Ok(ShaderFailureReason::UnexpectedClosing {
            construct: construct(kind)?,
        }),
        _ => Err("the test has no oracle for the diagnostic category".into()),
    }
}

fn construct(value: Construct) -> TestResult<ShaderConstruct> {
    Ok(match value {
        Construct::Brace => ShaderConstruct::Brace,
        Construct::Parenthesis => ShaderConstruct::Parenthesis,
        Construct::Quote => ShaderConstruct::Quote,
        Construct::Comment => ShaderConstruct::Comment,
        _ => return Err("the test has no oracle for the lexical construct".into()),
    })
}

fn native_budget(value: Budget) -> TestResult<ShaderBudget> {
    Ok(match value {
        Budget::SourceBytes => ShaderBudget::SourceBytes,
        Budget::TokenBytes => ShaderBudget::TokenBytes,
        Budget::ValueBytes => ShaderBudget::ValueBytes,
        Budget::Tokens => ShaderBudget::Tokens,
        Budget::Declarations => ShaderBudget::Declarations,
        Budget::Properties => ShaderBudget::Properties,
        Budget::Depth => ShaderBudget::Depth,
        _ => return Err("the test has no oracle for the exhausted parser limit".into()),
    })
}
