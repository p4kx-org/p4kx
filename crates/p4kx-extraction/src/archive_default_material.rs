// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex, PoisonError};

use p4kx_archive::{Archive, Entry, EntryOrigin};
use p4kx_model::{
    CompanionError, LayerCache, Material, MaterialError, MaterialLibrary, MaterialReading,
};
use sha2::{Digest as _, Sha256};

use super::{EntryChoice, unique_entry_choice};
use crate::model_companions::{ArchiveCompanions, MAX_MATERIAL_LEN};
use crate::sound::read_capped;
use crate::source_provenance::model_material_origins::MaterialObservation;
use crate::{ArchiveMaterialReads, MaterialArchiveOrigins, PathIndex, TextureVerdicts};

const DEFAULT_ENTRY: &str = "Data/Materials/material_default.mtl";

/// Outcome of selecting the physical fallback material from its owning Archive.
#[derive(Debug)]
#[non_exhaustive]
pub enum ArchiveDefaultMaterialState {
    /// The owning directory contains no matching Entry.
    Missing,
    /// More than one physical Entry matches the policy path.
    Ambiguous {
        /// Original evidence for two conflicting occurrences.
        witnesses: [EntryOrigin; 2],
    },
    /// The directory lookup itself could not establish a match.
    LookupFailed(CompanionError),
    /// The selected Entry could not be read and verified.
    Unreadable(CompanionError),
    /// Verified bytes failed the shared material parser.
    Malformed(MaterialError),
    /// The parsed library does not contain one drawable material.
    UnsupportedLibrary,
    /// A verified and supported original material is available.
    Resolved,
}

/// The observed default source and the policy outcome, independent of the authored library.
#[derive(Debug)]
pub struct ArchiveDefaultMaterial {
    original: Option<EntryOrigin>,
    state: ArchiveDefaultMaterialState,
    library: Option<MaterialLibrary>,
    origins: MaterialArchiveOrigins,
    decoded_sha256: Option<[u8; 32]>,
}

impl ArchiveDefaultMaterial {
    /// Returns the exact candidate, including a failed read or parse.
    #[must_use]
    pub const fn original(&self) -> Option<&EntryOrigin> {
        self.original.as_ref()
    }

    /// Returns the independent physical fallback lookup and read state.
    #[must_use]
    pub const fn state(&self) -> &ArchiveDefaultMaterialState {
        &self.state
    }

    /// Returns the SHA-256 of the verified, decoded original payload when it was read.
    #[must_use]
    pub const fn decoded_sha256(&self) -> Option<&[u8; 32]> {
        self.decoded_sha256.as_ref()
    }

    /// Returns the original resolved single-material library.
    #[must_use]
    pub const fn library(&self) -> Option<&MaterialLibrary> {
        self.library.as_ref()
    }

    /// Returns the selected material when the physical lookup supports using it.
    #[must_use]
    pub fn material(&self) -> Option<&Material> {
        self.library.as_ref()?.materials.first()
    }

    /// Returns the observed companions of the drawable fallback.
    #[must_use]
    pub const fn material_origins(&self) -> &MaterialArchiveOrigins {
        &self.origins
    }

    pub(crate) fn read(index: &PathIndex<'_>, verdicts: &TextureVerdicts) -> Self {
        let archive = index.archive();
        match unique_entry_choice(index, DEFAULT_ENTRY) {
            Ok(EntryChoice::Missing) => {
                Self::unavailable(None, ArchiveDefaultMaterialState::Missing)
            }
            Ok(EntryChoice::Unique(entry)) => Self::read_entry(index, entry, verdicts),
            Ok(EntryChoice::Ambiguous(first, second)) => Self::ambiguous(archive, first, second),
            Err(cause) => Self::unavailable(None, ArchiveDefaultMaterialState::LookupFailed(cause)),
        }
    }

    fn read_entry(index: &PathIndex<'_>, entry: &Entry, verdicts: &TextureVerdicts) -> Self {
        let archive = index.archive();
        let Some(original) = archive.entry_origin(entry) else {
            return Self::lost_origin();
        };
        let bytes = match verified_bytes(archive, entry) {
            Ok(bytes) => bytes,
            Err(cause) => {
                return Self::unavailable(
                    Some(original),
                    ArchiveDefaultMaterialState::Unreadable(cause),
                );
            }
        };
        let decoded_sha256 = Some(Sha256::digest(&bytes).into());
        let (parsed, origins) = parse_observed(index, entry, verdicts, &bytes);
        match parsed {
            Ok(library)
                if library.single
                    && library
                        .materials
                        .first()
                        .is_some_and(|material| !material.hidden()) =>
            {
                Self {
                    original: Some(original),
                    state: ArchiveDefaultMaterialState::Resolved,
                    library: Some(library),
                    origins,
                    decoded_sha256,
                }
            }
            Ok(_) => Self::read_but_unusable(
                original,
                ArchiveDefaultMaterialState::UnsupportedLibrary,
                decoded_sha256,
            ),
            Err(cause) => Self::read_but_unusable(
                original,
                ArchiveDefaultMaterialState::Malformed(cause),
                decoded_sha256,
            ),
        }
    }

    fn ambiguous(archive: &Archive, first: &Entry, second: &Entry) -> Self {
        match (archive.entry_origin(first), archive.entry_origin(second)) {
            (Some(first), Some(second)) => Self::unavailable(
                None,
                ArchiveDefaultMaterialState::Ambiguous {
                    witnesses: [first, second],
                },
            ),
            _ => Self::lost_origin(),
        }
    }

    fn lost_origin() -> Self {
        Self::unavailable(
            None,
            ArchiveDefaultMaterialState::LookupFailed(
                "selected default Entry lost its original owner".into(),
            ),
        )
    }

    fn read_but_unusable(
        original: EntryOrigin,
        state: ArchiveDefaultMaterialState,
        decoded_sha256: Option<[u8; 32]>,
    ) -> Self {
        Self {
            decoded_sha256,
            original: Some(original),
            state,
            library: None,
            origins: MaterialArchiveOrigins::default(),
        }
    }

    fn unavailable(original: Option<EntryOrigin>, state: ArchiveDefaultMaterialState) -> Self {
        Self {
            original,
            state,
            library: None,
            origins: MaterialArchiveOrigins::default(),
            decoded_sha256: None,
        }
    }
}

fn parse_observed(
    index: &PathIndex<'_>,
    entry: &Entry,
    verdicts: &TextureVerdicts,
    bytes: &[u8],
) -> (
    Result<MaterialLibrary, MaterialError>,
    MaterialArchiveOrigins,
) {
    let reads = ArchiveMaterialReads::default();
    let mut origins = MaterialArchiveOrigins::default();
    let mut resolver = ArchiveCompanions {
        index,
        folder: "",
        verdicts,
        model_sources: None,
        material_fields: None,
        material_observation: Some(MaterialObservation {
            reads: &reads,
            used: Some(&mut origins),
        }),
    };
    let parsed = MaterialLibrary::parse_with_resolver(
        DEFAULT_ENTRY,
        bytes,
        Some(entry.path().to_owned()),
        &mut resolver,
        MaterialReading::Drawn,
        &LayerCache::new(),
    );
    (parsed, origins)
}

fn verified_bytes(archive: &Archive, entry: &Entry) -> Result<Vec<u8>, CompanionError> {
    let size = entry.uncompressed_size();
    if size > MAX_MATERIAL_LEN {
        return Err(format!("default material exceeds {MAX_MATERIAL_LEN} bytes").into());
    }
    let reader = archive.open_entry_verified(entry)?;
    read_capped(reader, size, MAX_MATERIAL_LEN)?
        .ok_or_else(|| "default material exceeds its admitted byte limit".into())
}

#[derive(Debug, Default)]
pub(crate) struct Cache(Mutex<Option<CacheEntry>>);

#[derive(Debug)]
struct CacheEntry {
    generation: u64,
    read: Arc<ArchiveDefaultMaterial>,
}

impl Cache {
    pub(crate) fn resolve(
        &self,
        index: &PathIndex<'_>,
        verdicts: &TextureVerdicts,
    ) -> Arc<ArchiveDefaultMaterial> {
        let archive = index.archive();
        let mut cached = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            let generation = archive.key_generation();
            if let Some(prior) = cached
                .as_ref()
                .filter(|prior| prior.generation == generation)
            {
                return Arc::clone(&prior.read);
            }
            let read = Arc::new(ArchiveDefaultMaterial::read(index, verdicts));
            if generation == archive.key_generation() {
                *cached = Some(CacheEntry {
                    generation,
                    read: Arc::clone(&read),
                });
                drop(cached);
                return read;
            }
        }
    }
}
