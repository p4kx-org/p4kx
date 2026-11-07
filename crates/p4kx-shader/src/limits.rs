// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Per-source limits applied before token or declaration storage grows.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum original payload bytes.
    pub source_bytes: usize,
    /// Maximum bytes in one atom, quoted token or comment.
    pub token_bytes: usize,
    /// Maximum bytes in one declaration value.
    pub value_bytes: usize,
    /// Maximum stored lexical tokens, including separators.
    pub tokens: usize,
    /// Maximum declarations across all scopes.
    pub declarations: usize,
    /// Maximum braced declarations named Property across all scopes.
    pub properties: usize,
    /// Maximum combined brace and parenthesis nesting; the hard maximum is 128.
    pub depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 1 << 20,
            token_bytes: 64 << 10,
            value_bytes: 64 << 10,
            tokens: 65_536,
            declarations: 16_384,
            properties: 4_096,
            depth: 64,
        }
    }
}
