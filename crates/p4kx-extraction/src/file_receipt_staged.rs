// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::origin_file_io;
use crate::output_file::StagedOutput;
use crate::{
    ArchiveWriteProtection, CancelToken, ConflictPolicy, OriginArtifactBinding,
    OriginArtifactDigest,
};

use super::lock::{Contention, ReceiptLock, invalid, protection_error};
use super::ordinary::check_native_companion;
use super::target_path::{BindTarget, bind_target, is_companion_name};
use super::{ExistingCompanions, FileReceipt};

#[cfg(test)]
#[path = "file_receipt_staged_tests.rs"]
mod tests;

/// Artifact replacement and its separate receipt result.
#[derive(Debug)]
#[must_use]
pub(crate) enum StagedReceiptPublication {
    /// The artifact was not moved. The caller still owns its temporary file.
    Uncommitted(io::Error),
    /// The artifact was moved.
    Committed {
        /// The publication outcome, or none when a foreign companion was retained.
        receipt: Option<io::Result<()>>,
        /// The ephemeral lock retirement outcome.
        retirement: io::Result<()>,
    },
}

#[derive(Debug, thiserror::Error)]
#[error("cannot retire the staged publication lock: {cleanup}")]
pub(super) struct RetirementFailure {
    #[source]
    primary: io::Error,
    cleanup: io::Error,
}

impl<'source> FileReceipt<'source> {
    pub(crate) fn for_native_staged_replacement(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: CancelToken,
    ) -> io::Result<Self> {
        Self::bind(
            output,
            protection,
            protection,
            cancel,
            BindTarget::NativeReplacement,
        )
    }

    pub(super) const fn replacement_mode(&self) -> BindTarget {
        if self.native_entry {
            BindTarget::NativeReplacement
        } else {
            BindTarget::Replacement
        }
    }
}

impl FileReceipt<'_> {
    /// Commits an owned staged artifact and publishes its measured byte binding.
    /// The caller has decided to replace under its conflict policy and owns pre-commit cleanup.
    pub(crate) fn publish_staged(
        &self,
        staged: &StagedOutput<'_>,
        digest: OriginArtifactDigest,
        checkpoint: impl FnMut() -> io::Result<()>,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> StagedReceiptPublication {
        self.publish_staged_with_policy(
            staged,
            digest,
            ConflictPolicy::Overwrite,
            checkpoint,
            || {},
            describe,
        )
        .unwrap_or_else(|| {
            StagedReceiptPublication::Uncommitted(io::Error::other(
                "an unconditional staged replacement was kept",
            ))
        })
    }

    /// Commits a staged artifact or returns `None` when `Skip` keeps a competing artifact.
    /// Calls `invalidated` when the existing receipt has been invalidated.
    pub(crate) fn publish_staged_with_policy(
        &self,
        staged: &StagedOutput<'_>,
        digest: OriginArtifactDigest,
        policy: ConflictPolicy,
        mut checkpoint: impl FnMut() -> io::Result<()>,
        invalidated: impl FnOnce(),
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> Option<StagedReceiptPublication> {
        let held = match self.acquire(Contention::Wait) {
            Ok(held) => held,
            Err(error) => return Some(StagedReceiptPublication::Uncommitted(error)),
        };
        let protection = self.receipt_protection.or(self.protection);
        let mut check = || {
            checkpoint()?;
            if self.cancel.is_cancelled() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            Ok(())
        };
        match self.place_staged(&held, staged, policy, &mut check, invalidated) {
            Ok(Some(companion)) => {
                let receipt = match companion {
                    CompanionPlacement::Publish => {
                        Some(self.publish_companion_checked(&held, digest, check, describe))
                    }
                    CompanionPlacement::PreserveForeign => None,
                };
                let retirement = held.retire(protection);
                Some(StagedReceiptPublication::Committed {
                    receipt,
                    retirement,
                })
            }
            Ok(None) => held
                .retire(protection)
                .err()
                .map(StagedReceiptPublication::Uncommitted),
            Err(primary) => {
                let error = match held.retire(protection) {
                    Ok(()) => primary,
                    Err(cleanup) => io::Error::other(RetirementFailure { primary, cleanup }),
                };
                Some(StagedReceiptPublication::Uncommitted(error))
            }
        }
    }

    fn place_staged(
        &self,
        held: &ReceiptLock,
        staged: &StagedOutput<'_>,
        policy: ConflictPolicy,
        mut checkpoint: impl FnMut() -> io::Result<()>,
        invalidated: impl FnOnce(),
    ) -> io::Result<Option<CompanionPlacement>> {
        held.check(&self.cancel)?;
        staged.check().map_err(protection_error)?;
        let current = bind_target(
            staged.output(),
            self.protection,
            &self.cancel,
            self.replacement_mode(),
        )?;
        if (current.path.as_path(), current.leaf) != (self.artifact.as_path(), self.leaf) {
            return Err(invalid("the staged file names a different origin artifact"));
        }
        self.check_target()?;
        checkpoint()?;
        held.check(&self.cancel)?;
        if policy == ConflictPolicy::Skip {
            match fs::symlink_metadata(&self.artifact) {
                Ok(_) => return Ok(None),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if self.native_entry && self.artifact.file_name().is_some_and(is_companion_name) {
            check_native_companion(&self.artifact)?;
        }
        let companion = self.companion_placement()?;
        invalidated();
        checkpoint()?;
        if policy == ConflictPolicy::Skip {
            staged.check().map_err(protection_error)?;
            origin_file_io::install_without_replacing(staged, &self.artifact)
                .map(|installed| installed.then_some(companion))
        } else {
            origin_file_io::replace_staged(staged)?;
            Ok(Some(companion))
        }
    }

    fn companion_placement(&self) -> io::Result<CompanionPlacement> {
        if !self.native_entry {
            self.invalidate(ExistingCompanions::RequireOwned)?;
            return Ok(CompanionPlacement::Publish);
        }
        self.invalidate(ExistingCompanions::PreserveForeign)?;
        match fs::symlink_metadata(self.path()) {
            Ok(_) => Ok(CompanionPlacement::PreserveForeign),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(CompanionPlacement::Publish)
            }
            Err(error) => Err(error),
        }
    }
}

enum CompanionPlacement {
    Publish,
    PreserveForeign,
}

pub(crate) fn finish_staged_receipt(
    receipt: Option<io::Result<()>>,
    retirement: io::Result<()>,
) -> io::Result<()> {
    match (receipt.unwrap_or(Ok(())), retirement) {
        (Err(primary), Err(cleanup)) => {
            Err(io::Error::other(RetirementFailure { primary, cleanup }))
        }
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}
