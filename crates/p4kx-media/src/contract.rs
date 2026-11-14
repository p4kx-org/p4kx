// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Capability, Digest, Limits, MediaFormat, MediaKind, MediaVersion, RootSource, Source,
    Subresource,
};

/// Terminal projection status of one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Every field group claimed as projected by this provider was projected.
    Complete,
    /// Useful data survives with explicitly diagnosed loss.
    Partial,
    /// No usable projection was produced.
    Failed,
}

/// Severity attached to a stable diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informative source limitation without advertised projection loss.
    Info,
    /// Some advertised projection is unavailable.
    Warning,
    /// The record could not be projected.
    Error,
}

/// A source-local issue with an unambiguous location.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MediaIssue {
    /// Stable machine-readable diagnosis code.
    pub code: String,
    /// Field, section or object path within this source.
    pub location: String,
    /// Severity of the observation.
    pub severity: Severity,
    /// Technical detail, without invented fallback values.
    pub detail: String,
}

impl MediaIssue {
    /// Describes one stable source-local diagnosis without replacing source data.
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        location: impl Into<String>,
        severity: Severity,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            location: location.into(),
            severity,
            detail: detail.into(),
        }
    }
}

/// One JSONL metadata or subresource record.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MediaRecord {
    /// Provider output kind.
    pub kind: MediaKind,
    /// Physical source and exact preservation evidence.
    pub source: Source,
    /// Source-local ordered identity.
    pub subresource: Subresource,
    /// Projection result.
    pub status: Status,
    /// Provider-specific typed data described by its field-group contract.
    pub data: Value,
    /// Every issue affecting this projection.
    pub issues: Vec<MediaIssue>,
}

/// Actual observations in one explicitly named counting unit.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Tally {
    /// Units considered by the operation.
    pub enumerated: u64,
    /// Units written completely.
    pub written: u64,
    /// Units written with diagnosed projection loss.
    pub partial: u64,
    /// Units that failed.
    pub failed: u64,
    /// Units intentionally skipped by selection or provider applicability.
    pub skipped: u64,
}

/// Separate physical, metadata and converted-output accounting.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Counts {
    /// Physical Entry callbacks; root metadata is not an Entry.
    pub source_entries: Tally,
    /// Metadata and subresource records, including root metadata.
    pub records: Tally,
    /// Converted output artifacts; retained source bytes are excluded.
    pub converted_outputs: Tally,
}

/// A completed artifact described without loading its contents.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Artifact {
    /// Generation-relative artifact path.
    pub path: String,
    /// Length in bytes.
    pub size: u64,
    /// SHA-256 over all file bytes.
    pub digest: Digest,
}

/// One provider's stream, capability and actual counts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct KindSummary {
    /// Selected provider capability and version.
    pub provider: Capability,
    /// Actual counts with explicit units.
    pub counts: Counts,
    /// Completed metadata stream.
    pub artifact: Artifact,
}

/// The only root commit point of a complete media generation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct Index {
    /// Fixed media contract discriminator.
    pub format: MediaFormat,
    /// Fixed media contract version, independently of catalog schemas.
    pub format_version: MediaVersion,
    /// Whether all selected operations completed without projection loss.
    pub complete: bool,
    /// Whether traversal discovered every selected source in its configured scope.
    pub enumeration_complete: bool,
    /// Root Archive provenance.
    pub source: RootSource,
    /// SHA-256 content generation identifier.
    pub generation: String,
    /// Artifact base relative to this index.
    pub artifact_base: String,
    /// Applied safety bounds; the synchronous queue depth is one source.
    pub limits: Limits,
    /// Physical Entries seen and selected once across providers.
    pub source_entries: Tally,
    /// Selected provider streams in registration order.
    pub kinds: Vec<KindSummary>,
    /// Streaming identities and exact raw references for every selected retained source.
    pub source_manifest: Artifact,
    /// Streaming manifest of raw and converted artifact hashes.
    pub artifact_manifest: Artifact,
    /// Hashed field origins; missing historical metadata has unknown coverage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_origins: Option<crate::origin_contract::OriginReceipts>,
}
