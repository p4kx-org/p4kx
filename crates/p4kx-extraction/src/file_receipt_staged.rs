// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::origin_file_io;
use crate::output_file::StagedOutput;
use crate::{OriginArtifactBinding, OriginArtifactDigest};

use super::lock::{Contention, ReceiptLock, invalid, protection_error};
use super::target_path::{BindTarget, bind_target};
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
        /// The receipt publication outcome.
        receipt: io::Result<()>,
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

impl FileReceipt<'_> {
    /// Commits an owned staged artifact and publishes its measured byte binding.
    /// The caller has decided to replace under its conflict policy and owns pre-commit cleanup.
    pub(crate) fn publish_staged(
        &self,
        staged: &StagedOutput<'_>,
        digest: OriginArtifactDigest,
        mut checkpoint: impl FnMut() -> io::Result<()>,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> StagedReceiptPublication {
        let held = match self.acquire(Contention::Wait) {
            Ok(held) => held,
            Err(error) => return StagedReceiptPublication::Uncommitted(error),
        };
        let protection = self.receipt_protection.or(self.protection);
        let mut check = || {
            checkpoint()?;
            if self.cancel.is_cancelled() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            Ok(())
        };
        match self.replace_staged(&held, staged, &mut check) {
            Ok(()) => {
                let receipt = self.publish_companion_checked(&held, digest, check, describe);
                let retirement = held.retire(protection);
                StagedReceiptPublication::Committed {
                    receipt,
                    retirement,
                }
            }
            Err(primary) => {
                let error = match held.retire(protection) {
                    Ok(()) => primary,
                    Err(cleanup) => io::Error::other(RetirementFailure { primary, cleanup }),
                };
                StagedReceiptPublication::Uncommitted(error)
            }
        }
    }

    fn replace_staged(
        &self,
        held: &ReceiptLock,
        staged: &StagedOutput<'_>,
        checkpoint: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        held.check(&self.cancel)?;
        staged.check().map_err(protection_error)?;
        let current = bind_target(
            staged.output(),
            self.protection,
            &self.cancel,
            BindTarget::Replacement,
        )?;
        if (current.path.as_path(), current.leaf) != (self.artifact.as_path(), self.leaf) {
            return Err(invalid("the staged file names a different origin artifact"));
        }
        self.check_target()?;
        checkpoint()?;
        held.check(&self.cancel)?;
        self.invalidate(ExistingCompanions::RequireOwned)?;
        origin_file_io::replace_staged(staged)
    }
}
