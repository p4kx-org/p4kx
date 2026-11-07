// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Entry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Algorithm, ByteScope, Verification};

/// An explicitly scoped integrity value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Digest {
    /// Algorithm name, including the CRC polynomial where relevant.
    pub algorithm: Algorithm,
    /// Exact bytes over which the value was computed or declared.
    pub scope: ByteScope,
    /// Lower-case hexadecimal spelling.
    pub hex: String,
}

/// A physical container Entry in the ordered path from the root Archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct ContainerIdentity {
    /// Zero-based directory index in the immediately enclosing Archive.
    pub entry_index: usize,
    /// Reader-exposed display path, whose separators may already be normalised.
    pub path: String,
    /// Original non-UTF-8 path bytes when the display path is lossy.
    pub raw_path_hex: Option<String>,
}

/// Identity that remains distinct for duplicate names and nested containers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Identity {
    /// The complete root Archive file.
    Root,
    /// A physical Entry identified through each containing directory.
    Entry {
        /// Ordered outermost-to-innermost physical container identities.
        containers: Vec<ContainerIdentity>,
        /// Identity in the immediate directory.
        entry: ContainerIdentity,
    },
}

/// Retained exact decoded payload, addressed relative to its generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct RawReference {
    /// Relative artifact path.
    pub path: String,
    /// Decoded byte length.
    pub size: u64,
    /// SHA-256 of the entire decoded payload.
    pub digest: Digest,
}

/// Provenance shared by records from one physical input.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Source {
    /// SHA-256 of every byte of the root Archive file.
    pub root: Digest,
    /// Physical source identity.
    pub identity: Identity,
    /// Decoded Entry size, or root file size.
    pub size: u64,
    /// Directory-declared checksum, never substituted for a cryptographic digest.
    pub declared_integrity: Option<Digest>,
    /// Whether the retained payload passed the reader's available directory checks.
    pub verification: Verification,
    /// Exact decoded Entry payload when available.
    pub raw: Option<RawReference>,
}

/// Source-local identity of a metadata object or ordered subresource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Subresource {
    /// Provider-owned semantic namespace.
    pub namespace: String,
    /// Zero-based original ordinal path, including duplicate named objects.
    pub indices: Vec<u64>,
}

/// Root file provenance and the public Build declaration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct RootSource {
    /// Root file display path, using lossy Unicode conversion when needed.
    pub path: String,
    /// All-byte cryptographic identity.
    pub digest: Digest,
    /// File size in bytes.
    pub size: u64,
    /// Build fields as declared by the adjacent launcher manifest.
    pub build: Option<BuildProvenance>,
    /// Public source installation flags, absent when the layout provides none.
    pub installation_flags: Option<u8>,
    /// Total directory issues observed by the reader, including beyond its sample limit.
    pub directory_issue_count: usize,
    /// Reader-bounded issue samples in their exposed order.
    pub directory_issues: Vec<crate::DirectoryIssue>,
    /// Whether the samples include every issue counted by the reader.
    pub directory_issue_samples_complete: bool,
    /// Whether the reader has an intact source Entry inventory.
    pub enumeration_complete: bool,
}

/// Public launcher Build fields without inferred values.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct BuildProvenance {
    /// Branch declaration.
    pub branch: String,
    /// Version declaration.
    pub version: String,
    /// Perforce change declaration.
    pub change_number: String,
    /// Build identifier declaration.
    pub build_id: String,
    /// Optional public patch supplied by the caller.
    pub public_patch: Option<String>,
}

/// One retained source available to a provider during its callback.
#[non_exhaustive]
pub struct SourceContext<'a> {
    /// Complete provenance, including retained payload integrity.
    pub source: &'a Source,
    /// Public directory Entry metadata.
    pub entry: &'a Entry,
    /// Exact retained payload on disk; bounded readers must use their domain limits.
    pub payload: &'a Path,
}

impl ContainerIdentity {
    /// Copies physical identity after matching Entry metadata in the supplied directory.
    ///
    /// # Errors
    /// Returns a contract error when the Entry metadata does not match that directory.
    pub fn try_new_with_entry(archive: &Archive, entry: &Entry) -> Result<Self, crate::MediaError> {
        if archive.entries().get(entry.index()) != Some(entry) {
            return Err(crate::MediaError::Contract(
                "identity Entry metadata does not match the supplied Archive".into(),
            ));
        }
        Ok(identity(archive, entry))
    }
}

impl Subresource {
    /// Identifies an original ordered resource within the provider's semantic namespace.
    #[must_use]
    pub fn new(namespace: impl Into<String>, indices: Vec<u64>) -> Self {
        Self {
            namespace: namespace.into(),
            indices,
        }
    }
}

pub(crate) fn identity(archive: &Archive, entry: &Entry) -> ContainerIdentity {
    use std::fmt::Write as _;
    let raw_path_hex = archive.raw_path(entry.index()).map(|raw| {
        raw.iter()
            .fold(String::with_capacity(raw.len() * 2), |mut text, byte| {
                let _ignored = write!(text, "{byte:02x}");
                text
            })
    });
    ContainerIdentity {
        entry_index: entry.index(),
        path: entry.path().into(),
        raw_path_hex,
    }
}

pub(crate) fn root(archive: &Archive, digest: Digest, size: u64) -> RootSource {
    let build = archive.build().map(|build| BuildProvenance {
        branch: build.branch().into(),
        version: build.version().into(),
        change_number: build.change_number().into(),
        build_id: build.build_id().into(),
        public_patch: build.public_patch().map(str::to_owned),
    });
    RootSource {
        path: archive.path().to_string_lossy().into_owned(),
        digest,
        size,
        build,
        installation_flags: archive.installation_status().raw_flags(),
        directory_issue_count: archive.issue_count(),
        directory_issues: archive
            .issues()
            .iter()
            .map(|(index, issue)| crate::directory_issue::project(*index, issue))
            .collect(),
        directory_issue_samples_complete: archive.issue_count() == archive.issues().len(),
        enumeration_complete: !archive
            .issues()
            .iter()
            .any(|(_, issue)| issue.is_archive_wide()),
    }
}

pub(crate) fn entry(
    root: &Digest,
    archive: &Archive,
    entry: &Entry,
    chain: &[ContainerIdentity],
) -> Result<Source, crate::MediaError> {
    let mut source = Source {
        root: root.clone(),
        identity: Identity::Entry {
            containers: chain.to_vec(),
            entry: identity(archive, entry),
        },
        size: entry.uncompressed_size(),
        declared_integrity: None,
        verification: Verification::NotAvailable,
        raw: None,
    };
    let checksum = entry.checksum();
    let (algorithm, scope) = match checksum {
        p4kx_extraction::Checksum::Crc32(_) => (Algorithm::Crc32, ByteScope::DecodedEntryPayload),
        p4kx_extraction::Checksum::Crc32c(_) => (Algorithm::Crc32c, ByteScope::DecodedEntryPayload),
        p4kx_extraction::Checksum::Sha256(_) => (Algorithm::Sha256, ByteScope::StoredEntryPayload),
        _ => {
            return Err(crate::error::at(
                &source,
                crate::MediaError::Contract("unsupported directory integrity algorithm".into()),
            ));
        }
    };
    source.declared_integrity = Some(Digest {
        algorithm,
        scope,
        hex: checksum.hex(),
    });
    source.verification = Verification::NotPerformed;
    Ok(source)
}

pub(crate) fn retained(source: &mut Source, entry: &Entry, raw: RawReference) {
    source.verification = if matches!(entry.checksum(), p4kx_extraction::Checksum::Sha256(_))
        && !entry.is_sha256_verified()
    {
        Verification::NotPerformed
    } else {
        Verification::Verified
    };
    source.raw = Some(raw);
}
