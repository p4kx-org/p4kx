// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, FieldOrigin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Artifact, MediaKind};

/// The closed field-origin transport contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub enum OriginProtocol {
    /// Shared original inventories and occurrence-scoped immediate origins.
    #[serde(rename = "p4kx.field-origins/1")]
    V1,
}

/// One payload stream whose individual occurrences have separate origins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum OriginStream {
    /// One provider-kind JSONL stream.
    Records {
        /// Provider output kind.
        kind: MediaKind,
    },
    /// Retained physical source rows.
    Sources,
    /// Actual artifact-manifest row occurrences.
    Artifacts,
    /// Canonical nullable Index.source.build projection, separately from the Index file.
    IndexBuild,
}

/// The exact stream-local position of an emitted payload row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct OriginScope {
    /// Exact payload stream.
    pub stream: OriginStream,
    /// Contiguous zero-based row position.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub ordinal: DecimalU64,
}

/// The producer-owned field mapping used for one occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum OriginProfile {
    /// No complete producer-owned field mapping is claimed.
    Unmigrated,
    /// Public Archive Entry metadata.
    ArchiveEntry,
    /// Original Archive layout and directory declarations.
    ArchiveLayout,
    /// Successfully decoded shader definition header.
    ShaderDefinition,
    /// Original shader Property declaration.
    ShaderProperty,
    /// Original shader declaration.
    ShaderDeclaration,
    /// Shader parser failure after consuming actual source bytes.
    ShaderFailure,
    /// Source-size failure derived from directory metadata before shader parsing.
    ShaderSourceLimit,
    /// Reader configuration refusal before source-content inspection.
    ShaderConfigurationFailure,
    /// Original loose WEM summary, chunk, opaque class or parser diagnosis.
    AudioWem,
    /// Original soundbank section, media and object observations.
    AudioBank,
    /// Original ATL XML and CryXml source observations.
    AudioControl,
    /// Original CAF, DBA and AIM inventory and decoded channels.
    Animation,
    /// Original XML animation definition syntax and source links.
    AnimationDefinition,
    /// Original model and geometry chunk table observations.
    Model,
    /// Original DDS header words and their normalized topology.
    TextureHeader,
    /// Physical DDS family members and expected split relationships.
    TextureFamily,
    /// Emitted DDS images and their exact per-surface source spans.
    TextureConversion,
    /// Successfully parsed material library and its actually used companions.
    MaterialLibrary,
    /// One header or typed row from an original material library.
    MaterialPart,
    /// Material parser failure after actual source-content inspection.
    MaterialFailure,
    /// Material source-size refusal before content inspection.
    MaterialSourceLimit,
    /// Invalid material reader configuration without a content contribution.
    MaterialConfigurationFailure,
    /// Original manifest-backed Build members within the Archive-root record.
    ArchiveBuild,
    /// Original manifest-backed nullable Build projection in the Index.
    IndexBuild,
}

/// Exact occurrence and field counts for one payload stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct OriginStreamSummary {
    /// Payload stream being counted.
    pub stream: OriginStream,
    /// Actual emitted rows, including repeated equal artifacts.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub occurrences: DecimalU64,
    /// Actual addressed JSON nodes.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub fields: DecimalU64,
}

/// Orthogonal source-mapping outcomes for one payload occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct OriginCoverage {
    /// All addressed nodes.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub fields: DecimalU64,
    /// Nodes with complete contributing-use sets.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub complete: DecimalU64,
    /// Nodes with explicitly incomplete contributing-use sets.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub incomplete: DecimalU64,
    /// Nodes whose encryption marker is unknown.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub unknown: DecimalU64,
    /// Nodes with a proved encrypted contributor.
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
    pub encrypted: DecimalU64,
}

/// Hashed shared inventories and field origins bound into the generation identity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct OriginReceipts {
    /// Closed transport capability.
    pub protocol: OriginProtocol,
    /// Original shared `PhysicalSource` rows.
    pub sources: Artifact,
    /// Original shared `SourceUseDescription` rows.
    pub uses: Artifact,
    /// Occurrence, field, completion and EOF rows.
    pub fields: Artifact,
    /// Complete multiplicities for every payload stream.
    pub streams: Vec<OriginStreamSummary>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum OriginRow<'a> {
    Occurrence {
        scope: &'a OriginScope,
        profile: OriginProfile,
        payload_sha256: &'a str,
    },
    Field {
        scope: &'a OriginScope,
        field: &'a str,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
        cuts: &'a [crate::origin_contributions::OriginCut],
    },
    End {
        scope: &'a OriginScope,
        coverage: &'a OriginCoverage,
    },
    Eof {
        streams: &'a [OriginStreamSummary],
    },
}
