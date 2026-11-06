// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ReleaseAsset;

/// A published Release of P4KX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The version without the leading v of its tag.
    pub version: String,
    /// The web page of the Release.
    pub page: String,
    /// A compatible package link, when package identity and asset metadata are valid.
    pub asset: Option<ReleaseAsset>,
}

impl Release {
    /// Returns the compatible package link or the Release page fallback.
    #[must_use]
    pub fn link(&self) -> &str {
        self.asset.as_ref().map_or(&self.page, |asset| &asset.url)
    }
}
