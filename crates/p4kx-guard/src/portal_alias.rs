// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The host identity established for a Documents-portal path.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub enum PortalAlias {
    /// The path is outside the Documents portal.
    NotPortal,
    /// The portal authenticated the document's host path.
    Resolved(HostAlias),
    /// Available evidence does not establish a host identity.
    Unknown(UnknownAlias),
}

/// An authenticated host path; installation membership remains a separate check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAlias {
    path: PathBuf,
    document_id: String,
    grant: DocumentGrant,
}

impl HostAlias {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) const fn new(path: PathBuf, document_id: String, grant: DocumentGrant) -> Self {
        Self {
            path,
            document_id,
            grant,
        }
    }

    /// Returns the authenticated host spelling, including validated descendants.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the exact document identifier used in the authenticated query.
    #[must_use]
    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    /// Returns whether the exported document is a file or a directory.
    #[must_use]
    pub const fn grant(&self) -> DocumentGrant {
        self.grant
    }
}

/// The filesystem object exposed by a document grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DocumentGrant {
    /// A single file without access to its host siblings.
    File,
    /// A directory whose validated descendants can be mapped.
    Directory,
}

/// Why a Documents-portal path could not be authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AliasUncertainty {
    /// Kernel mount evidence is unavailable or incomplete.
    #[error("mount inventory unavailable")]
    MountInventory,
    /// The observed mount does not match the authenticated Documents service.
    #[error("untrusted document mount")]
    UntrustedMount,
    /// A source or transport work limit was exceeded.
    #[error("alias evidence exceeds its limit")]
    Limit,
    /// A document identifier or suffix could escape its grant.
    #[error("invalid document path")]
    DocumentPath,
    /// A file grant was used as the parent of another path.
    #[error("file grant has descendants")]
    FileDescendant,
    /// Existing symbolic links prevent a proved descendant mapping.
    #[error("document descendant is a symbolic link")]
    SymbolicLink,
    /// Required filesystem identity cannot be observed.
    #[error("document filesystem identity unavailable")]
    Filesystem,
    /// Portal proxy identities cannot prove distinct host files for an existing output.
    #[error("host file identity unavailable")]
    HostFileIdentity,
    /// An output no longer matches its exclusively created descriptor or namespace.
    #[error("owned output identity changed")]
    OwnedOutputIdentity,
    /// A local authenticated session connection is unavailable.
    #[error("session bus unavailable")]
    SessionBus,
    /// The Documents service has no authenticated owner.
    #[error("Documents service unavailable")]
    ServiceUnavailable,
    /// The service owner changed during the query.
    #[error("Documents service owner changed")]
    OwnerChanged,
    /// The service rejected access to the document.
    #[error("document access denied")]
    AccessDenied,
    /// The service does not provide authenticated host-path lookup.
    #[error("Documents interface {0} does not provide host paths")]
    UnsupportedInterface(u32),
    /// An expected method is absent despite the advertised interface.
    #[error("Documents method unavailable")]
    UnsupportedMethod,
    /// The complete operation or one method exceeded its deadline.
    #[error("Documents query timed out")]
    Timeout,
    /// The caller cancelled before the query completed.
    #[error("Documents query cancelled")]
    Cancelled,
    /// The service returned an invalid or ambiguous response.
    #[error("malformed Documents response")]
    MalformedReply,
    /// The reply did not contain the requested document.
    #[error("document host path unavailable")]
    MissingDocument,
}

/// An uncertainty result preserving local failures without copying remote error text.
#[derive(Debug, Clone, thiserror::Error)]
#[error("cannot establish the portal alias: {reason}")]
pub struct UnknownAlias {
    reason: AliasUncertainty,
    #[source]
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl UnknownAlias {
    pub(crate) const fn new(reason: AliasUncertainty) -> Self {
        Self {
            reason,
            source: None,
        }
    }

    pub(crate) fn with_source(
        reason: AliasUncertainty,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            reason,
            source: Some(Arc::new(source)),
        }
    }

    /// Returns the typed evidence failure without inferring an installation.
    #[must_use]
    pub const fn reason(&self) -> AliasUncertainty {
        self.reason
    }
}
