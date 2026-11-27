// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourcedValue};

/// One original Locale token and its admitted lookup selector.
#[derive(Debug)]
pub(crate) struct AuthoredLocaleInput<'source> {
    key: SourcedValue<&'source str>,
    selector: Option<SourceUse>,
}

impl<'source> AuthoredLocaleInput<'source> {
    pub(super) const fn new(key: SourcedValue<&'source str>, selector: Option<SourceUse>) -> Self {
        Self { key, selector }
    }

    pub(crate) const fn key(&self) -> &SourcedValue<&'source str> {
        &self.key
    }

    pub(crate) const fn selector(&self) -> Option<&SourceUse> {
        self.selector.as_ref()
    }
}
