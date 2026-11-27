// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

/// A logical path checkpoint restored after the current actual value is complete.
#[derive(Clone, Copy, Default)]
pub(super) struct PathMark {
    text_len: usize,
}

/// One reusable logical path for original diagnostics and exact numeric receipts.
#[derive(Default)]
pub(super) struct OriginalPath {
    text: String,
}

impl OriginalPath {
    pub(super) const fn mark(&self) -> PathMark {
        PathMark {
            text_len: self.text.len(),
        }
    }

    pub(super) fn field(&mut self, name: &str) {
        if !self.text.is_empty() {
            self.text.push('.');
        }
        self.text.push_str(name);
    }

    pub(super) fn element(&mut self, ordinal: u64) -> Result<(), std::fmt::Error> {
        write!(self.text, "[{ordinal}]")
    }

    pub(super) fn restore(&mut self, mark: PathMark) {
        self.text.truncate(mark.text_len);
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }
}
