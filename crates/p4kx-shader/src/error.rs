// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::Span;

/// Bounded resource whose next unit cannot be accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Budget {
    /// Original payload bytes.
    SourceBytes,
    /// Bytes of one lexical token or comment.
    TokenBytes,
    /// Bytes of one declaration value.
    ValueBytes,
    /// Stored lexical tokens.
    Tokens,
    /// Ordered declarations.
    Declarations,
    /// Braced Property declarations.
    Properties,
    /// Combined braces and parentheses.
    Depth,
}

/// Lexical construct whose closing delimiter is absent or mismatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Construct {
    /// Block comment.
    Comment,
    /// Quoted value.
    Quote,
    /// Braced body.
    Brace,
    /// Parenthesised value.
    Parenthesis,
}

/// Source-local reason that structural parsing cannot complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SyntaxErrorKind {
    /// Input cannot be interpreted as supported UTF-8 text.
    #[error("unknown or invalid source encoding")]
    UnknownEncoding,
    /// A lexical construct has no closing delimiter.
    #[error("unterminated {0:?}")]
    Unterminated(Construct),
    /// A closing delimiter does not match its containing scope.
    #[error("unexpected closing {0:?}")]
    UnexpectedClosing(Construct),
    /// A configured bound would be exceeded.
    #[error("{budget:?} limit {limit} exceeded")]
    Limit {
        /// Resource being bounded.
        budget: Budget,
        /// Actual configured bound.
        limit: usize,
    },
    /// A collection could not reserve the next bounded element.
    #[error("bounded parser storage could not be allocated")]
    Allocation,
}

/// Structural failure with offsets in the original decoded payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("{kind} at bytes {start}..{end}", start=.span.start, end=.span.end_exclusive)]
#[non_exhaustive]
pub struct SyntaxError {
    /// Precise structural or resource failure.
    pub kind: SyntaxErrorKind,
    /// Source-local half-open byte range.
    pub span: Span,
}

impl SyntaxError {
    pub(crate) const fn new(kind: SyntaxErrorKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub(crate) const fn limit(budget: Budget, limit: usize, span: Span) -> Self {
        Self::new(SyntaxErrorKind::Limit { budget, limit }, span)
    }
}

/// Structural failure or the unchanged caller checkpoint error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReadError<E> {
    /// The source cannot be parsed under the declared grammar and limits.
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
    /// The caller interrupted the read, preserving its original failure.
    #[error("shader read checkpoint failed: {0}")]
    Interrupted(#[source] E),
}
