// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use crate::query::lexer::Keyword;

/// What the parser expected at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expected {
    /// `from`, `at` or `referencing` at the start.
    Selection,
    /// A struct name after `from`.
    TypeName,
    /// A string after `at`.
    Glob,
    /// A string after `referencing`.
    Referent,
    /// A field name in a path.
    FieldName,
    /// What may follow `from <type>`.
    AfterFrom,
    /// What may follow `at "<glob>"`.
    AfterAt,
    /// What may follow `referencing "<record>"`.
    AfterReferencing,
    /// What may follow a path.
    AfterPath,
    /// A comparison, `not` or `(` where a condition starts.
    Condition,
    /// A comparison operator after the path of a comparison.
    Operator,
    /// `with` after `starts`.
    With,
    /// A number, a string, `true` or `false` after a comparison operator.
    Value,
    /// A string after a matching operator.
    Text,
    /// What may follow a condition in parentheses.
    CloseParenthesis,
    /// What may follow the condition of `where`.
    AfterCondition,
    /// An index, `*` or a type name after `[`.
    Index,
    /// `]` after an index, `*` or a type name.
    CloseBracket,
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Selection => "`from`, `at` or `referencing`",
            Self::TypeName => "a type name",
            Self::Glob => "a record-path glob in double quotes",
            Self::Referent => "a GUID or record path in double quotes",
            Self::FieldName => "a field name",
            Self::AfterFrom => "`at`, `referencing`, `select`, `where` or the end of the Query",
            Self::AfterAt => "`referencing`, `select`, `where` or the end of the Query",
            Self::AfterReferencing => "`select`, `where` or the end of the Query",
            Self::AfterPath => "`.`, `[`, `->`, `,`, `where` or the end of the Query",
            Self::Condition => "a comparison, `not` or `(`",
            Self::Operator => {
                "`=`, `!=`, `<`, `<=`, `>`, `>=`, `contains`, `starts with` or `like`"
            }
            Self::With => "`with`",
            Self::Value => "a number, a string, `true` or `false`",
            Self::Text => "a string in double quotes",
            Self::CloseParenthesis => "`and`, `or` or `)`",
            Self::AfterCondition => "`and`, `or` or the end of the Query",
            Self::Index => "an index, `*` or a type name",
            Self::CloseBracket => "`]`",
        })
    }
}

/// The comparison literal category for [`super::QueryErrorKind::IncompatibleComparison`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LiteralKind {
    /// A number without fraction or exponent.
    Integer,
    /// A number with a fraction or an exponent.
    Fraction,
    /// A string.
    Text,
    /// `true` or `false`.
    Bool,
}

impl fmt::Display for LiteralKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Integer => "an integer",
            Self::Fraction => "a number with a fraction or exponent",
            Self::Text => "a string",
            Self::Bool => "a boolean",
        })
    }
}

/// The token the parser found instead of an expected one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Found {
    /// The end of the Query.
    End,
    /// A keyword.
    Keyword(Keyword),
    /// A name.
    Name(String),
    /// A string.
    Text,
    /// A `,`.
    Comma,
    /// A `.`.
    Dot,
    /// A number.
    Number,
    /// An operator or a parenthesis.
    Symbol(&'static str),
}

impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::End => f.write_str("the end of the Query"),
            Self::Keyword(keyword) => write!(f, "the keyword `{keyword}`"),
            Self::Name(name) => write!(f, "the name `{name}`"),
            Self::Text => f.write_str("a string"),
            Self::Comma => f.write_str("`,`"),
            Self::Dot => f.write_str("`.`"),
            Self::Number => f.write_str("a number"),
            Self::Symbol(symbol) => write!(f, "`{symbol}`"),
        }
    }
}
