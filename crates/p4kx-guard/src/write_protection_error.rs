// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use crate::{UnknownAlias, WriteRefused};

/// A refused write with its established installation or identity boundary.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum WriteProtectionError {
    /// An Archive-backed output job supplied no source identity.
    #[error("[p4kx.write.source-identity] Archive output requires at least one source")]
    MissingSource,
    /// The target identifies an input Archive itself.
    #[error("[p4kx.write.source-overwrite] output would replace an input Archive")]
    SourceOverwrite {
        /// The input Archive as supplied by the caller.
        archive: PathBuf,
        /// The requested output path.
        target: PathBuf,
    },
    /// The target is inside an established game installation.
    #[error(transparent)]
    Installation(#[from] WriteRefused),
    /// The target's host identity could not be established.
    #[error("[p4kx.write.target-identity] write target identity unavailable: {cause}")]
    TargetIdentity {
        /// The requested output path.
        target: PathBuf,
        /// The failed identity evidence.
        #[source]
        cause: UnknownAlias,
    },
    /// The Archive source's host identity could not be established.
    #[error("[p4kx.write.source-identity] Archive source identity unavailable: {cause}")]
    SourceIdentity {
        /// The requested source path.
        archive: PathBuf,
        /// The failed identity evidence.
        #[source]
        cause: UnknownAlias,
    },
    /// A file-only grant cannot establish the source parent's installation membership.
    #[error(
        "[p4kx.write.source-membership] write refused within the Archive source parent: installation membership unavailable"
    )]
    SourceMembership {
        /// The Archive path as supplied by the caller.
        archive: PathBuf,
        /// The requested output path.
        target: PathBuf,
        /// The authenticated host parent whose membership is unknown.
        parent: PathBuf,
    },
}

impl WriteProtectionError {
    /// Returns the established installation refusal, if one was observed.
    #[must_use]
    pub const fn installation(&self) -> Option<&WriteRefused> {
        match self {
            Self::Installation(refused) => Some(refused),
            Self::MissingSource
            | Self::SourceOverwrite { .. }
            | Self::TargetIdentity { .. }
            | Self::SourceIdentity { .. }
            | Self::SourceMembership { .. } => None,
        }
    }

    /// Returns the requested target, or `None` when source setup failed first.
    #[must_use]
    pub fn target(&self) -> Option<&Path> {
        match self {
            Self::Installation(refused) => Some(&refused.target),
            Self::TargetIdentity { target, .. }
            | Self::SourceMembership { target, .. }
            | Self::SourceOverwrite { target, .. } => Some(target),
            Self::MissingSource | Self::SourceIdentity { .. } => None,
        }
    }

    /// Returns whether cancellation stopped identity lookup before a write.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        match self {
            Self::TargetIdentity { cause, .. } | Self::SourceIdentity { cause, .. } => {
                matches!(cause.reason(), crate::AliasUncertainty::Cancelled)
            }
            Self::Installation(_)
            | Self::MissingSource
            | Self::SourceMembership { .. }
            | Self::SourceOverwrite { .. } => false,
        }
    }
}
