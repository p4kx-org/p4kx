// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{Hash, Hasher};

/// A name matched using the source's ASCII case-insensitive comparison.
#[derive(Clone, Copy, Debug, Eq)]
pub(crate) struct AsciiKey<'source>(pub(crate) &'source str);

impl<'source> AsciiKey<'source> {
    pub(crate) fn locale(key: &'source str) -> Self {
        Self(key.strip_prefix('@').unwrap_or(key))
    }
}

impl PartialEq for AsciiKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(other.0)
    }
}

impl Hash for AsciiKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.len().hash(state);
        for byte in self.0.bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}
