// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use p4kx_archive::{EntryOrigin, EntryOriginStep, SourceEncryption};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::archive_material_reference::{ArchiveDefaultMaterial, ArchiveDefaultMaterialState};

/// Physical fallback lookup and its original Archive source evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DefaultMaterialReceipt {
    /// Outcome of the physical source lookup.
    pub state: DefaultMaterialState,
    /// Original Entry when the lookup found one unique candidate.
    pub source: Option<DefaultMaterialSource>,
    /// Two original witnesses when more than one Entry matched.
    pub ambiguous_sources: Vec<DefaultMaterialSource>,
    /// SHA-256 of the verified decoded source when its payload could be read.
    pub decoded_sha256: Option<String>,
}

/// Outcome of looking up a physical Archive default material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DefaultMaterialState {
    /// A verified drawable material is available.
    Resolved,
    /// The owning Archive has no matching Entry.
    Missing,
    /// More than one physical Entry matches the policy path.
    Ambiguous,
    /// The lookup could not establish original ownership.
    LookupFailed,
    /// The selected Entry could not be read and verified.
    Unreadable,
    /// The verified payload could not be parsed as a material.
    Malformed,
    /// The parsed library does not contain one drawable material.
    UnsupportedLibrary,
}

/// Original material Entry and its containing Archive Entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DefaultMaterialSource {
    /// Original directory ordinal in the immediate owner.
    pub entry_index: usize,
    /// Original stored Entry path.
    pub entry_path: String,
    /// Container Entry coordinates, outermost first, with unknown coordinates explicit.
    pub containers: Vec<Option<DefaultMaterialContainer>>,
    /// Effective encryption evidence across the original Entry and its known containers.
    pub contains_encrypted_source: Option<bool>,
}

/// Original directory coordinate of a containing Archive Entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DefaultMaterialContainer {
    /// Original directory ordinal in the immediate owner.
    pub entry_index: usize,
    /// Original stored Entry path.
    pub entry_path: String,
}

impl ArchiveDefaultMaterial {
    /// Returns the shared serializable evidence for this lookup.
    #[must_use]
    pub fn receipt(&self) -> DefaultMaterialReceipt {
        let mut ambiguous_sources = Vec::new();
        let state = match self.state() {
            ArchiveDefaultMaterialState::Resolved => DefaultMaterialState::Resolved,
            ArchiveDefaultMaterialState::Missing => DefaultMaterialState::Missing,
            ArchiveDefaultMaterialState::Ambiguous { witnesses } => {
                ambiguous_sources.extend(witnesses.iter().map(DefaultMaterialSource::of));
                DefaultMaterialState::Ambiguous
            }
            ArchiveDefaultMaterialState::LookupFailed(_) => DefaultMaterialState::LookupFailed,
            ArchiveDefaultMaterialState::Unreadable(_) => DefaultMaterialState::Unreadable,
            ArchiveDefaultMaterialState::Malformed(_) => DefaultMaterialState::Malformed,
            ArchiveDefaultMaterialState::UnsupportedLibrary => {
                DefaultMaterialState::UnsupportedLibrary
            }
        };
        DefaultMaterialReceipt {
            state,
            source: self.original().map(DefaultMaterialSource::of),
            ambiguous_sources,
            decoded_sha256: self.decoded_sha256().map(sha256_hex),
        }
    }
}

impl DefaultMaterialSource {
    fn of(origin: &EntryOrigin) -> Self {
        Self {
            entry_index: origin.entry().index(),
            entry_path: origin.entry().path().to_owned(),
            containers: origin
                .ancestors()
                .map(|step| step.map(DefaultMaterialContainer::of))
                .collect(),
            contains_encrypted_source: match origin.encryption() {
                SourceEncryption::Encrypted => Some(true),
                SourceEncryption::Unencrypted => Some(false),
                _ => None,
            },
        }
    }
}

impl DefaultMaterialContainer {
    fn of(step: &EntryOriginStep) -> Self {
        Self {
            entry_index: step.index(),
            entry_path: step.path().to_owned(),
        }
    }
}

fn sha256_hex(digest: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(2 * digest.len());
    for byte in digest {
        let _ignored = write!(hex, "{byte:02x}");
    }
    hex
}
