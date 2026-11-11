// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::Location;
use serde::{Serialize, Serializer};

/// A retained view of the decoder's immutable original instance location text.
#[derive(Debug)]
pub(crate) struct OriginalInstanceLocation<'source> {
    location: Arc<Location<'source>>,
}

impl<'source> OriginalInstanceLocation<'source> {
    pub(super) const fn new(location: Arc<Location<'source>>) -> Self {
        Self { location }
    }
}

impl AsRef<str> for OriginalInstanceLocation<'_> {
    fn as_ref(&self) -> &str {
        self.location.as_str()
    }
}

impl Serialize for OriginalInstanceLocation<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_ref())
    }
}
