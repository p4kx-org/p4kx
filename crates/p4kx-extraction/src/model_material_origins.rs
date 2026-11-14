// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};

use p4kx_archive::EntryOrigin;
use p4kx_model::{MaterialCompanionUse, MaterialReadOrigin, MaterialStatus};

/// Resolves original Archive Entries for opaque reads cached by one model provider.
#[derive(Debug, Default)]
pub struct ArchiveMaterialReads(Mutex<HashMap<MaterialReadOrigin, EntryOrigin>>);

impl ArchiveMaterialReads {
    pub(crate) fn register(&self, entry: EntryOrigin) -> MaterialReadOrigin {
        let token = MaterialReadOrigin::default();
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(token.clone(), entry);
        token
    }

    pub(crate) fn origin(&self, token: &MaterialReadOrigin) -> Option<EntryOrigin> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(token)
            .cloned()
    }
}

pub(crate) struct MaterialObservation<'a> {
    pub(crate) reads: &'a ArchiveMaterialReads,
    pub(crate) used: Option<&'a mut MaterialArchiveOrigins>,
}

/// The physical Archive entry and material position a layer lookup actually selected.
#[derive(Debug)]
pub struct MaterialLayerOrigin {
    entry: EntryOrigin,
    source_material: Option<usize>,
}

impl MaterialLayerOrigin {
    /// Returns the admitted Archive Entry supplying this layer lookup.
    #[must_use]
    pub const fn entry(&self) -> &EntryOrigin {
        &self.entry
    }

    /// Returns the resolved material's position in the layer file.
    #[must_use]
    pub const fn source_material(&self) -> Option<usize> {
        self.source_material
    }
}

/// The admitted library and layer reads for one material result.
#[derive(Debug, Default)]
pub struct MaterialArchiveOrigins {
    library: Option<EntryOrigin>,
    layers: BTreeMap<(usize, usize), MaterialLayerOrigin>,
}

impl MaterialArchiveOrigins {
    pub(crate) fn record(&mut self, usage: MaterialCompanionUse, entry: EntryOrigin) {
        match usage {
            MaterialCompanionUse::Library => self.library = Some(entry),
            MaterialCompanionUse::Layer {
                material,
                layer,
                source_material,
            } => {
                self.layers.insert(
                    (material, layer),
                    MaterialLayerOrigin {
                        entry,
                        source_material,
                    },
                );
            }
            _ => {}
        }
    }

    /// Returns the physical library selected by the resolver, including a failed attempt.
    #[must_use]
    pub const fn library(&self) -> Option<&EntryOrigin> {
        self.library.as_ref()
    }

    /// Returns the physical Entry and selection of one attempted layer.
    #[must_use]
    pub fn layer(&self, material: usize, layer: usize) -> Option<&MaterialLayerOrigin> {
        self.layers.get(&(material, layer))
    }
}

/// A material result paired with the Archive Entries of the resolver's actual reads.
#[derive(Debug)]
#[non_exhaustive]
pub struct ArchiveMaterialRead {
    /// The result of reading and drawing the material library.
    pub status: MaterialStatus,
    /// The physical library and layer observations.
    pub origins: MaterialArchiveOrigins,
}

impl ArchiveMaterialRead {
    /// Pairs the material result with unknown original-source evidence.
    #[must_use]
    pub fn unobserved(status: MaterialStatus) -> Self {
        Self {
            status,
            origins: MaterialArchiveOrigins::default(),
        }
    }
}
