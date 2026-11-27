// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::CompanionError;

/// A validated material request and its optional resolver-owned destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialRequest {
    path: String,
    target: Option<String>,
    search_sibling: bool,
}

#[derive(Debug, Error)]
#[error("the path leaves the Data folder or is no plain path")]
struct InvalidMaterialRequest;

impl MaterialRequest {
    /// Validates an ordinary relative material name.
    ///
    /// # Errors
    /// Returns an error for a name outside the relative material-path contract.
    pub fn new(name: &str) -> Result<Self, CompanionError> {
        let path = crate::mtl_load::request_path(name).ok_or(InvalidMaterialRequest)?;
        Ok(Self {
            path,
            target: None,
            search_sibling: true,
        })
    }

    /// Restricts lookup to the logical Data root.
    #[must_use]
    pub const fn at_data_root(mut self) -> Self {
        self.search_sibling = false;
        self
    }

    /// Returns whether a bare request searches beside the model first.
    #[must_use]
    pub const fn searches_sibling(&self) -> bool {
        self.search_sibling
    }

    /// Records the destination established by the owning resolver.
    #[must_use]
    pub fn with_target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }

    /// Returns the relative material request without its extension.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Consumes the request and returns its resolver-owned destination.
    #[must_use]
    pub fn into_target(self) -> Option<String> {
        self.target
    }
}
