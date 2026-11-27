// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::WriteProtectionError;

pub(crate) fn host_target(
    target: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteProtectionError> {
    if cancel.load(Ordering::Acquire) {
        return Err(WriteProtectionError::TargetIdentity {
            target: target.to_path_buf(),
            cause: crate::UnknownAlias::new(crate::AliasUncertainty::Cancelled),
        });
    }
    #[cfg(target_os = "linux")]
    {
        let result = match crate::resolve_portal_alias(target, cancel) {
            crate::PortalAlias::NotPortal => crate::portal_filesystem::absolute(target)
                .and_then(|absolute| crate::portal_filesystem::existing_ancestor(&absolute)),
            crate::PortalAlias::Resolved(host) => Ok(host.path().to_path_buf()),
            crate::PortalAlias::Unknown(cause) => Err(cause),
        };
        result.map_err(|cause| WriteProtectionError::TargetIdentity {
            target: target.to_path_buf(),
            cause,
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let absolute =
            std::path::absolute(target).map_err(|error| WriteProtectionError::TargetIdentity {
                target: target.to_path_buf(),
                cause: crate::UnknownAlias::with_source(crate::AliasUncertainty::Filesystem, error),
            })?;
        Ok(crate::installation::resolve(&absolute))
    }
}

pub(crate) fn cleanup_host_target(
    target: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteProtectionError> {
    #[cfg(target_os = "linux")]
    {
        let ordinary =
            crate::portal_native::ordinary_target_for_cleanup(target).map_err(|cause| {
                WriteProtectionError::TargetIdentity {
                    target: target.to_path_buf(),
                    cause,
                }
            })?;
        ordinary.map_or_else(|| host_target(target, cancel), Ok)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cancel;
        std::path::absolute(target)
            .map(|absolute| crate::installation::resolve(&absolute))
            .map_err(|error| WriteProtectionError::TargetIdentity {
                target: target.to_path_buf(),
                cause: crate::UnknownAlias::with_source(crate::AliasUncertainty::Filesystem, error),
            })
    }
}
