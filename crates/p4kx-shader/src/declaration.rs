// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Span;

/// Lexical shape of a declaration, without interpreting unknown source constructs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Form {
    /// A bare name without an authored value.
    Marker,
    /// A name followed by an equals sign and an optional value.
    Equals,
    /// A name followed by a balanced parenthesised value.
    Parentheses,
    /// An optional name followed by a balanced braced body.
    Block,
    /// A source-spanned construct without a supported declaration shape.
    Unknown,
}

/// One ordered declaration, retaining duplicate names and unknown syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Declaration {
    /// Complete authored construct, excluding inter-declaration whitespace.
    pub span: Span,
    /// Original name token, absent for unnamed or unrecognised constructs.
    pub name: Option<Span>,
    /// Authored syntactic form.
    pub form: Form,
    /// Original value range; an explicit empty value has equal offsets.
    pub value: Option<Span>,
    /// Sole atom or quoted token of a value, excluding surrounding trivia.
    pub literal: Option<Span>,
    /// Braced child declarations in original order.
    pub children: Vec<Self>,
}
