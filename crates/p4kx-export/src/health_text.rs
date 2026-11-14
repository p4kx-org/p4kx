// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Borrow;
use std::ops::Deref;
use std::sync::Arc;

use serde::{Serialize, Serializer};

/// Immutable text storage without original-input authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HealthText(Arc<str>);

impl HealthText {
    pub(crate) fn new(value: &str) -> Self {
        Self(Arc::from(value))
    }

    /// Borrows the complete retained text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for HealthText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for HealthText {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for HealthText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
