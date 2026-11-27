// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::shader_word::ShaderWord;

/// Explicit interpretation outcome of one authored Mask occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ShaderMask {
    /// A literal in the supported explicit-hexadecimal or unambiguous-zero profile.
    Parsed {
        /// Exact unsigned 64-bit source word as decimal text.
        word: ShaderWord,
    },
    /// The occurrence is a marker or block, rather than a value declaration.
    MissingValue,
    /// A value declaration is explicitly empty.
    EmptyValue,
    /// A nonzero unprefixed numeric token has no established radix.
    UnknownRadix,
    /// A symbolic, quoted or compound expression remains uninterpreted.
    UnsupportedExpression,
    /// An explicit hexadecimal token has invalid digits or no digits.
    Malformed,
    /// An explicit hexadecimal value exceeds this reader's 64-bit support.
    Overflow,
    /// A newer reader outcome has no supported typed mapping.
    UnsupportedReaderVariant {
        /// Exact diagnostic variant spelling, without evaluating its source expression.
        detail: String,
    },
}

impl ShaderMask {
    pub(crate) fn from_reader(mask: p4kx_shader::Mask<'_>) -> Self {
        use p4kx_shader::MaskIssue;

        match mask.interpreted {
            Ok(value) => Self::Parsed {
                word: ShaderWord::new(value),
            },
            Err(MaskIssue::MissingValue) => Self::MissingValue,
            Err(MaskIssue::EmptyValue) => Self::EmptyValue,
            Err(MaskIssue::UnknownRadix) => Self::UnknownRadix,
            Err(MaskIssue::UnsupportedExpression) => Self::UnsupportedExpression,
            Err(MaskIssue::Malformed) => Self::Malformed,
            Err(MaskIssue::Overflow) => Self::Overflow,
            Err(other) => Self::UnsupportedReaderVariant {
                detail: format!("{other:?}"),
            },
        }
    }
}
