// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
#[cfg(any(target_os = "linux", test))]
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::archive_source_identity::ArchiveSourceIdentity;
use crate::{Installation, WriteProtectionError};

/// Immutable source-origin evidence carried by one Archive-backed write job.
#[derive(Debug, Clone)]
pub struct ArchiveWriteProtection {
    job: Arc<()>,
    sources: Vec<SourceBoundary>,
}

#[derive(Debug, Clone)]
struct SourceBoundary {
    identity: ArchiveSourceIdentity,
    membership: Membership,
}

#[derive(Debug, Clone)]
enum Membership {
    Known(Installation),
    #[cfg(any(target_os = "linux", test))]
    UncertainParent {
        archive: PathBuf,
        parent: PathBuf,
    },
}

impl ArchiveWriteProtection {
    /// Establishes source identity without treating a file-only grant as an installation.
    ///
    /// # Errors
    /// Returns source-identity uncertainty when the source cannot be authenticated.
    pub fn new(source: &Path, cancel: &AtomicBool) -> Result<Self, WriteProtectionError> {
        Self::for_sources(&[source], cancel)
    }

    /// Establishes every Archive origin consumed by one output job.
    ///
    /// # Errors
    /// Returns missing-source or origin-identity refusal without creating output.
    pub fn for_sources(
        sources: &[&Path],
        cancel: &AtomicBool,
    ) -> Result<Self, WriteProtectionError> {
        if sources.is_empty() {
            return Err(WriteProtectionError::MissingSource);
        }
        let sources = sources
            .iter()
            .map(|source| SourceBoundary::new(source, cancel))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            job: Arc::new(()),
            sources,
        })
    }

    /// Checks one output against global protection and every retained source boundary.
    ///
    /// # Errors
    /// Returns an installation or distinct source/target identity refusal.
    pub fn ensure_writable(
        &self,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        let host = crate::write_target::host_target(target, cancel)?;
        crate::registry::check_known(&host)?;
        self.check_host(target, &host)
    }

    pub(crate) fn ensure_cleanup_writable(
        &self,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        let host = crate::write_target::cleanup_host_target(target, cancel)?;
        crate::registry::check_known(&host)?;
        self.check_host(target, &host)
    }

    fn check_host(&self, target: &Path, host: &Path) -> Result<(), WriteProtectionError> {
        self.sources
            .iter()
            .try_for_each(|source| source.check_host(target, host))
    }

    pub(crate) fn same_job(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.job, &other.job)
    }

    pub(crate) fn ensure_owned_host(
        &self,
        target: &Path,
        host: &Path,
        token: &crate::OwnedFileToken,
    ) -> Result<(), WriteProtectionError> {
        if !token.belongs_to(self) {
            return Err(crate::owned_file_token::changed(target));
        }
        crate::registry::check_known(host)?;
        for source in &self.sources {
            source.identity.check_owned(target, host)?;
            source.check_membership(target, host)?;
        }
        Ok(())
    }

    pub(crate) fn check_namespace_host(
        &self,
        target: &Path,
        host: &Path,
    ) -> Result<(), WriteProtectionError> {
        for source in &self.sources {
            source.identity.check_namespace(target, host)?;
            source.check_membership(target, host)?;
        }
        Ok(())
    }

    pub(crate) fn check_directory_root(
        &self,
        target: &Path,
        host: &Path,
    ) -> Result<(), WriteProtectionError> {
        self.check_namespace_host(target, host)?;
        self.sources
            .iter()
            .try_for_each(|source| source.identity.check_root(target, host))
    }

    #[cfg(test)]
    fn from_alias(source: &Path, alias: crate::PortalAlias) -> Result<Self, WriteProtectionError> {
        Ok(Self {
            job: Arc::new(()),
            sources: vec![SourceBoundary::from_alias(source, alias)?],
        })
    }
}

impl SourceBoundary {
    fn new(source: &Path, cancel: &AtomicBool) -> Result<Self, WriteProtectionError> {
        if cancel.load(Ordering::Acquire) {
            return Err(WriteProtectionError::SourceIdentity {
                archive: source.to_path_buf(),
                cause: crate::UnknownAlias::new(crate::AliasUncertainty::Cancelled),
            });
        }
        #[cfg(target_os = "linux")]
        {
            Self::from_alias(source, crate::resolve_portal_alias(source, cancel))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::known(source, source)
        }
    }

    fn known(source: &Path, host: &Path) -> Result<Self, WriteProtectionError> {
        let identity = ArchiveSourceIdentity::new(source, host)?;
        let mut protected = Installation::new();
        if let Some(parent) = identity.host().parent()
            && parent.join("Bin64").is_dir()
        {
            protected.add(parent);
        }
        Ok(Self {
            identity,
            membership: Membership::Known(protected),
        })
    }

    #[cfg(any(target_os = "linux", test))]
    fn from_alias(source: &Path, alias: crate::PortalAlias) -> Result<Self, WriteProtectionError> {
        match alias {
            crate::PortalAlias::NotPortal => Self::known(source, source),
            crate::PortalAlias::Resolved(host) => {
                if host.grant() == crate::DocumentGrant::Directory {
                    Self::directory_grant(source, host.path())
                } else {
                    let parent = host.path().parent().ok_or_else(|| {
                        WriteProtectionError::SourceIdentity {
                            archive: source.to_path_buf(),
                            cause: crate::UnknownAlias::new(crate::AliasUncertainty::DocumentPath),
                        }
                    })?;
                    Ok(Self {
                        identity: ArchiveSourceIdentity::documents(source, host.path())?,
                        membership: Membership::UncertainParent {
                            archive: source.to_path_buf(),
                            parent: parent.to_path_buf(),
                        },
                    })
                }
            }
            crate::PortalAlias::Unknown(cause) => Err(WriteProtectionError::SourceIdentity {
                archive: source.to_path_buf(),
                cause,
            }),
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn directory_grant(source: &Path, host: &Path) -> Result<Self, WriteProtectionError> {
        let parent = host
            .parent()
            .ok_or_else(|| WriteProtectionError::SourceIdentity {
                archive: source.to_path_buf(),
                cause: crate::UnknownAlias::new(crate::AliasUncertainty::DocumentPath),
            })?;
        let membership = crate::source_membership::contains_installation(parent).or_else(|_| {
            let visible = source
                .parent()
                .ok_or_else(|| crate::UnknownAlias::new(crate::AliasUncertainty::Filesystem))?;
            crate::source_membership::contains_installation(visible)
        });
        let membership = membership.map_or_else(
            |_| Membership::UncertainParent {
                archive: source.to_path_buf(),
                parent: parent.to_path_buf(),
            },
            |present| {
                let mut installation = Installation::new();
                if present {
                    installation.add(parent);
                }
                Membership::Known(installation)
            },
        );
        Ok(Self {
            identity: ArchiveSourceIdentity::documents(source, host)?,
            membership,
        })
    }

    fn check_host(&self, target: &Path, host: &Path) -> Result<(), WriteProtectionError> {
        self.identity.check(target, host)?;
        self.check_membership(target, host)
    }

    fn check_membership(&self, target: &Path, host: &Path) -> Result<(), WriteProtectionError> {
        match &self.membership {
            Membership::Known(installation) => installation.check(host).map_err(|mut refused| {
                refused.target = target.to_path_buf();
                refused.into()
            }),
            #[cfg(any(target_os = "linux", test))]
            Membership::UncertainParent { archive, parent } => {
                if host.starts_with(parent) {
                    return Err(WriteProtectionError::SourceMembership {
                        archive: archive.clone(),
                        target: target.to_path_buf(),
                        parent: parent.clone(),
                    });
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;
