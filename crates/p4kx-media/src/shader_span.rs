// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::MediaError;

/// Origin of every shader byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderOffsetBasis {
    /// Start of the original decoded Entry payload, including any byte-order marker.
    Entry,
}

/// Half-open original Entry byte offsets, independent of decoded character counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderSpan {
    /// Start of the decoded Entry payload.
    pub basis: ShaderOffsetBasis,
    /// First included byte.
    pub start: u64,
    /// First excluded byte.
    pub end_exclusive: u64,
}

impl ShaderSpan {
    pub(crate) fn from_reader(span: p4kx_shader::Span) -> Result<Self, MediaError> {
        Self::new(span.start, span.end_exclusive)
    }

    pub(crate) fn new(start: usize, end_exclusive: usize) -> Result<Self, MediaError> {
        if start > end_exclusive {
            return Err(MediaError::Contract("shader byte range is reversed".into()));
        }
        Ok(Self {
            basis: ShaderOffsetBasis::Entry,
            start: u64::try_from(start)
                .map_err(|_| MediaError::Contract("shader byte offset exceeds u64".into()))?,
            end_exclusive: u64::try_from(end_exclusive)
                .map_err(|_| MediaError::Contract("shader byte offset exceeds u64".into()))?,
        })
    }
}

/// Exact source text of one bounded name, value or literal token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderText {
    /// Range into the retained decoded source.
    pub span: ShaderSpan,
    /// Exact original text in that range, without escape or whitespace normalisation.
    pub text: String,
}
