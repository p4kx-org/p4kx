// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::{self, Display, Formatter};
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
    #[error(
        "[p4kx.documents.unsupported-interface:{0}] Documents interface {0} does not provide host paths"
    )]
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

#[cfg(test)]
mod prerequisite_marker_tests {
    use super::{AliasUncertainty, UnknownAlias};

    #[test]
    fn an_unsupported_interface_reports_its_exact_unsigned_version() {
        for version in [0, 4, 5, u32::MAX] {
            let reason = AliasUncertainty::UnsupportedInterface(version);
            let expected = format!("[p4kx.documents.unsupported-interface:{version}]");
            assert!(
                reason.to_string().starts_with(expected.as_str()),
                "typed version {version}"
            );
            let failure = UnknownAlias::new(reason);
            assert_eq!(failure.reason(), reason, "typed cause is retained");
            assert!(
                failure.to_string().contains(expected.as_str()),
                "wrapped marker is retained"
            );
        }
    }

    #[test]
    fn transport_and_other_failures_never_emit_an_unsupported_version_marker() {
        for reason in [
            AliasUncertainty::SessionBus,
            AliasUncertainty::ServiceUnavailable,
            AliasUncertainty::OwnerChanged,
            AliasUncertainty::AccessDenied,
            AliasUncertainty::UnsupportedMethod,
            AliasUncertainty::Timeout,
            AliasUncertainty::Cancelled,
            AliasUncertainty::MalformedReply,
            AliasUncertainty::Filesystem,
            AliasUncertainty::Limit,
        ] {
            assert!(
                !UnknownAlias::new(reason)
                    .to_string()
                    .contains("[p4kx.documents.unsupported-interface:"),
                "{reason:?}"
            );
        }
    }
}

/// An uncertainty result preserving local failures without copying remote error text.
#[derive(Debug, Clone)]
pub struct UnknownAlias {
    reason: AliasUncertainty,
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl Display for UnknownAlias {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot establish the portal alias: {}",
            self.reason
        )
    }
}

impl Error for UnknownAlias {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
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

#[cfg(test)]
mod source_tests {
    use std::error::Error;
    use std::io;

    use super::{AliasUncertainty, UnknownAlias};

    #[derive(Debug, thiserror::Error)]
    #[error("synthetic filesystem cause")]
    struct Marker;

    #[test]
    fn shared_local_failures_expose_the_original_io_error_and_payload() {
        let failure = UnknownAlias::with_source(
            AliasUncertainty::MountInventory,
            io::Error::new(io::ErrorKind::PermissionDenied, Marker),
        );
        for observed in [failure.clone(), failure] {
            let source = observed.source().expect("a local cause is retained");
            let original = source
                .downcast_ref::<io::Error>()
                .expect("the standard error chain exposes the actual I/O error");
            assert_eq!(original.kind(), io::ErrorKind::PermissionDenied);
            assert!(
                original
                    .get_ref()
                    .is_some_and(<dyn Error + Send + Sync>::is::<Marker>),
                "the original typed payload is retained without text conversion"
            );
        }
    }

    #[test]
    fn generic_local_causes_and_absent_causes_keep_their_contract() {
        let failure = UnknownAlias::with_source(AliasUncertainty::Filesystem, Marker);
        assert!(
            failure.source().is_some_and(<dyn Error>::is::<Marker>),
            "the source is the original concrete error"
        );
        assert!(
            UnknownAlias::new(AliasUncertainty::Cancelled)
                .source()
                .is_none(),
            "a typed refusal does not invent a local cause"
        );
    }
}
