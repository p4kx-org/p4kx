// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::{Digest, Identity, MediaKind, OriginalIdentifier, Source, Subresource};

/// A reader-proven target identity without a projection or preservation claim.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct ReferenceTarget {
    /// Root Archive cryptographic identity.
    pub root: Digest,
    /// Exact ordered container and physical Entry identity.
    pub source: Identity,
    /// Owning provider kind for a particular metadata/subresource target.
    pub kind: Option<MediaKind>,
    /// Target subresource when the reader proves one.
    pub subresource: Option<Subresource>,
}

impl ReferenceTarget {
    /// Identifies a target in an observed physical source.
    #[must_use]
    pub fn new_with_source(source: &Source) -> Self {
        Self {
            root: source.root.clone(),
            source: source.identity.clone(),
            kind: None,
            subresource: None,
        }
    }

    /// Identifies a kind-qualified original subresource in an observed source.
    #[must_use]
    pub fn new_with_subresource(
        source: &Source,
        kind: MediaKind,
        subresource: Subresource,
    ) -> Self {
        Self {
            root: source.root.clone(),
            source: source.identity.clone(),
            kind: Some(kind),
            subresource: Some(subresource),
        }
    }
}

/// Why an existing target was outside the selected projection scope.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum NotSelectedReason {
    /// Include or exclusion Filters did not select it.
    Filter,
    /// Its provider kind was not requested.
    KindNotRequested,
}

/// Why target resolution could not establish a usable answer.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ReferenceUnavailable {
    /// The target's required provider is not present.
    ProviderMissing,
    /// Target metadata has not been read.
    TargetNotRead,
    /// The source identifier cannot be interpreted.
    InvalidIdentifier,
    /// The relevant candidate inventory is incomplete.
    IncompleteInventory,
    /// The reader does not support the reference layout.
    UnsupportedLayout,
    /// The target source cannot be read.
    SourceUnavailable,
    /// Candidate enumeration exceeded its configured bound.
    CandidateLimit {
        /// Candidate limit actually applied.
        limit: u32,
    },
    /// Candidate identity metadata exceeded its configured byte bound.
    CandidateBytes {
        /// Byte limit actually applied.
        limit: u64,
    },
}

/// A reader's known selection or availability state of a proven candidate.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CandidateState {
    /// The target belongs to the selected source and kind scope.
    Selected,
    /// The target exists outside that scope.
    NotSelected {
        /// Why it was not selected.
        reason: NotSelectedReason,
    },
    /// The target identity exists but its metadata is unavailable.
    Unavailable {
        /// Why it cannot be used.
        reason: ReferenceUnavailable,
    },
}

/// One actual domain-reader match, retaining order and duplicate declarations.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct ReferenceCandidate {
    /// Proven physical and optional subresource identity.
    pub target: ReferenceTarget,
    /// Reader-reported selection or availability.
    pub state: CandidateState,
}

impl ReferenceCandidate {
    /// Couples a proven target with its reader-observed selection or availability.
    #[must_use]
    pub const fn new(target: ReferenceTarget, state: CandidateState) -> Self {
        Self { target, state }
    }
}

/// Whether the domain reader completed its candidate enumeration.
#[derive(Debug, Clone, Copy)]
pub enum CandidateInventory {
    /// The supplied iterator enumerates every actual match.
    Complete,
    /// Only observed candidates, if any, can be supplied.
    Unavailable(ReferenceUnavailable),
}

/// Central reference classification, separate from source preservation and typed coverage.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ReferenceResolution {
    /// Exactly one selected target identity was proved.
    Resolved {
        /// Unique target, without claiming its projection or raw preservation complete.
        target: ReferenceTarget,
    },
    /// A complete lookup proved no matching target.
    Missing,
    /// A complete lookup proved more than one match, in original reader order.
    Ambiguous {
        /// Every actual candidate, including duplicate declarations.
        candidates: Vec<ReferenceCandidate>,
    },
    /// Exactly one target exists outside the selected projection scope.
    NotSelected {
        /// Proven target identity.
        target: ReferenceTarget,
        /// Why it was not selected.
        reason: NotSelectedReason,
    },
    /// The lookup cannot claim a complete resolution.
    Unavailable {
        /// Typed reason for unavailable resolution.
        reason: ReferenceUnavailable,
        /// Bounded observed candidates, without implying that this list is complete.
        known_candidates: Vec<ReferenceCandidate>,
    },
}

/// Original source identifier and an honestly bounded resolution result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Reference {
    pub(crate) original: OriginalIdentifier,
    pub(crate) resolution: ReferenceResolution,
    pub(crate) observed_candidates: u64,
    pub(crate) candidates_complete: bool,
}

impl Reference {
    /// Returns the unmodified original source identifier.
    #[must_use]
    pub const fn original(&self) -> &OriginalIdentifier {
        &self.original
    }

    /// Returns the centrally classified resolution state.
    #[must_use]
    pub const fn resolution(&self) -> &ReferenceResolution {
        &self.resolution
    }

    /// Returns the number of candidate iterator elements actually observed.
    #[must_use]
    pub const fn observed_candidates(&self) -> u64 {
        self.observed_candidates
    }

    /// Returns whether enumeration proved that every candidate was observed and retained.
    #[must_use]
    pub const fn candidates_complete(&self) -> bool {
        self.candidates_complete
    }
}
