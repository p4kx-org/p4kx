// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

/// A source selection or geometry-presence invariant without private display values.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SourceCheck {
    SourceIdentity,
    SourceScope,
    ModelPresence,
}

impl fmt::Display for SourceCheck {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "verification_{self:?}")
    }
}

impl std::error::Error for SourceCheck {}

pub(crate) fn require<E: std::error::Error + 'static>(
    condition: bool,
    check: E,
) -> super::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Box::new(check))
    }
}
