// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Budget, Construct, SyntaxError, SyntaxErrorKind};

use crate::shader_declaration::ShaderMeaning;
use crate::shader_mask::ShaderMask;
use crate::shader_payload::{ShaderBudget, ShaderConstruct, ShaderFailureReason};
use crate::shader_values;
use crate::{MediaError, MediaIssue, Severity, Status};

pub(crate) fn failure(error: SyntaxError) -> Result<ShaderFailureReason, MediaError> {
    Ok(match error.kind {
        SyntaxErrorKind::UnknownEncoding => ShaderFailureReason::UnknownEncoding,
        SyntaxErrorKind::Unterminated(construct) => construct_reason(construct, true),
        SyntaxErrorKind::UnexpectedClosing(construct) => construct_reason(construct, false),
        SyntaxErrorKind::Limit { budget, limit } => match budget_kind(budget) {
            Some(budget) => ShaderFailureReason::Limit {
                budget,
                limit: shader_values::count(limit)?,
            },
            None => ShaderFailureReason::UnsupportedReaderVariant {
                detail: format!("{budget:?}"),
            },
        },
        SyntaxErrorKind::Allocation => ShaderFailureReason::Allocation,
        other => ShaderFailureReason::UnsupportedReaderVariant {
            detail: format!("{other:?}"),
        },
    })
}

fn construct_reason(construct: Construct, unterminated: bool) -> ShaderFailureReason {
    let kind = match construct {
        Construct::Comment => ShaderConstruct::Comment,
        Construct::Quote => ShaderConstruct::Quote,
        Construct::Brace => ShaderConstruct::Brace,
        Construct::Parenthesis => ShaderConstruct::Parenthesis,
        other => {
            return ShaderFailureReason::UnsupportedReaderVariant {
                detail: format!("{other:?}"),
            };
        }
    };
    if unterminated {
        ShaderFailureReason::Unterminated { construct: kind }
    } else {
        ShaderFailureReason::UnexpectedClosing { construct: kind }
    }
}

const fn budget_kind(budget: Budget) -> Option<ShaderBudget> {
    match budget {
        Budget::SourceBytes => Some(ShaderBudget::SourceBytes),
        Budget::TokenBytes => Some(ShaderBudget::TokenBytes),
        Budget::ValueBytes => Some(ShaderBudget::ValueBytes),
        Budget::Tokens => Some(ShaderBudget::Tokens),
        Budget::Declarations => Some(ShaderBudget::Declarations),
        Budget::Properties => Some(ShaderBudget::Properties),
        Budget::Depth => Some(ShaderBudget::Depth),
        _ => None,
    }
}

pub(crate) fn mask_status(meaning: &ShaderMeaning) -> (Status, Vec<MediaIssue>) {
    let ShaderMeaning::Mask { interpretation } = meaning else {
        return (Status::Complete, Vec::new());
    };
    let (code, detail, loss) = match interpretation {
        ShaderMask::Parsed { .. } => return (Status::Complete, Vec::new()),
        ShaderMask::MissingValue => (
            "shader.mask.missing-value",
            "Authored Mask has no value form; its occurrence remains exact.",
            false,
        ),
        ShaderMask::EmptyValue => (
            "shader.mask.empty-value",
            "Authored Mask value is explicitly empty; no integer is fabricated.",
            false,
        ),
        ShaderMask::UnknownRadix => (
            "shader.mask.unknown-radix",
            "The unprefixed numeric token has no proven source radix and remains raw.",
            false,
        ),
        ShaderMask::UnsupportedExpression => (
            "shader.mask.unsupported-expression",
            "The symbolic, quoted or compound value is retained without evaluation.",
            false,
        ),
        ShaderMask::Malformed => (
            "shader.mask.malformed",
            "The explicit hexadecimal token has missing or invalid digits within the supported literal profile.",
            true,
        ),
        ShaderMask::Overflow => (
            "shader.mask.supported-width-exceeded",
            "The value lies outside the supported unsigned64 interpretation width and remains raw; no engine-invalidity claim is made.",
            false,
        ),
        ShaderMask::UnsupportedReaderVariant { .. } => (
            "shader.mask.unsupported-reader-variant",
            "The preserved reader outcome has no supported typed projection.",
            true,
        ),
    };
    let (status, severity) = if loss {
        (Status::Partial, Severity::Warning)
    } else {
        (Status::Complete, Severity::Info)
    };
    (
        status,
        vec![MediaIssue::new(code, "declaration.mask", severity, detail)],
    )
}
