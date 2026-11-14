// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogSummary;

/// A built Basetool catalog: its JSON and its summary.
#[derive(Debug, Clone)]
pub struct BuiltCatalog {
    pub(crate) json: Vec<u8>,
    pub(crate) summary: CatalogSummary,
}

impl BuiltCatalog {
    /// Returns the catalog as UTF-8 JSON ending in a newline.
    #[must_use]
    pub fn json(&self) -> &[u8] {
        &self.json
    }

    /// Returns the summary.
    #[must_use]
    pub const fn summary(&self) -> &CatalogSummary {
        &self.summary
    }

    /// Returns the JSON and the summary.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, CatalogSummary) {
        (self.json, self.summary)
    }
}
