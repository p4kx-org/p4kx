// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::IntErrorKind;

use thiserror::Error;

use crate::{Field, Form};

/// Why a preserved Mask declaration has no interpreted unsigned 64-bit word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MaskIssue {
    /// The field is a bare marker or block rather than a value declaration.
    #[error("mask has no value form")]
    MissingValue,
    /// An explicit value declaration contains no lexical value.
    #[error("mask has an explicit empty value")]
    EmptyValue,
    /// Nonzero unprefixed numeric text has no proven source radix.
    #[error("mask radix is not declared")]
    UnknownRadix,
    /// The value is a symbolic, quoted or compound expression.
    #[error("mask expression is retained without evaluation")]
    UnsupportedExpression,
    /// An explicitly hexadecimal token has invalid or absent digits.
    #[error("invalid hexadecimal mask token")]
    Malformed,
    /// The explicitly hexadecimal token exceeds the reader's 64-bit supported word width.
    #[error("mask exceeds the supported unsigned 64-bit width")]
    Overflow,
}

/// Raw authored mask and its explicitly limited integer interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Mask<'source> {
    /// Complete raw value range, including authored whitespace and comments.
    pub raw: Option<&'source str>,
    /// Exact unsigned word, never a floating-point JSON number or a fallback zero.
    pub interpreted: Result<u64, MaskIssue>,
}

impl<'source> Mask<'source> {
    pub(crate) fn new(field: Field<'source>) -> Self {
        let raw = field.value();
        let interpreted = if matches!(
            field.declaration().form,
            Form::Marker | Form::Block | Form::Unknown
        ) {
            Err(MaskIssue::MissingValue)
        } else {
            parse(field.literal(), raw)
        };
        Self { raw, interpreted }
    }
}

fn parse(literal: Option<&str>, raw: Option<&str>) -> Result<u64, MaskIssue> {
    let Some(literal) = literal else {
        return Err(if raw.is_some_and(|raw| raw.trim().is_empty()) {
            MaskIssue::EmptyValue
        } else {
            MaskIssue::UnsupportedExpression
        });
    };
    if literal == "0" {
        return Ok(0);
    }
    let digits = literal
        .strip_prefix("0x")
        .or_else(|| literal.strip_prefix("0X"));
    if let Some(digits) = digits {
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(MaskIssue::Malformed);
        }
        return u64::from_str_radix(digits, 16).map_err(|error| match error.kind() {
            IntErrorKind::PosOverflow => MaskIssue::Overflow,
            _ => MaskIssue::Malformed,
        });
    }
    Err(if literal.bytes().all(|byte| byte.is_ascii_digit()) {
        MaskIssue::UnknownRadix
    } else {
        MaskIssue::UnsupportedExpression
    })
}
