// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::CompanionError;

/// The opaque identity a resolver attaches to one physical material read.
#[derive(Debug, Clone)]
pub struct MaterialReadOrigin(Arc<()>);

impl Default for MaterialReadOrigin {
    fn default() -> Self {
        Self(Arc::new(()))
    }
}

impl PartialEq for MaterialReadOrigin {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for MaterialReadOrigin {}

impl Hash for MaterialReadOrigin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// The bytes and optional resolver identity for an admitted material lookup.
pub struct MaterialCompanion {
    /// The fetched bytes or original lookup failure.
    pub bytes: Result<Option<Vec<u8>>, CompanionError>,
    /// The physical read identity, including an attempted read when available.
    pub origin: Option<MaterialReadOrigin>,
}

impl MaterialCompanion {
    /// Returns a material lookup without physical origin evidence.
    #[must_use]
    pub fn unobserved(bytes: Result<Option<Vec<u8>>, CompanionError>) -> Self {
        Self {
            bytes,
            origin: None,
        }
    }
}

/// Where the material reader used a physical companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialCompanionUse {
    /// The selected material library.
    Library,
    /// A layer looked up for a material slot.
    Layer {
        /// The material's position in its library.
        material: usize,
        /// The layer's position in the material.
        layer: usize,
        /// The selected material position in the layer file, if it supplied the surface.
        source_material: Option<usize>,
    },
}
