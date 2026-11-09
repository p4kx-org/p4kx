// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

pub(crate) const MANIFEST_NAME: &str = "build_manifest.id";

/// Opaque identity and byte count of one completed build-manifest read.
#[derive(Debug, Clone)]
pub struct BuildManifestOrigin {
    input: Arc<ManifestInput>,
}

#[derive(Debug)]
struct ManifestInput {
    name: &'static str,
    byte_length: u64,
}

impl BuildManifestOrigin {
    pub(crate) fn new(byte_length: u64) -> Self {
        Self {
            input: Arc::new(ManifestInput {
                name: MANIFEST_NAME,
                byte_length,
            }),
        }
    }

    /// Returns the manifest's stored basename without its host directory.
    #[must_use]
    pub fn name(&self) -> &str {
        self.input.name
    }

    /// Returns the byte count of the completed original read.
    #[must_use]
    pub fn byte_length(&self) -> u64 {
        self.input.byte_length
    }

    /// Returns whether both handles identify the same completed read.
    #[must_use]
    pub fn is_same_input(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.input, &other.input)
    }
}
