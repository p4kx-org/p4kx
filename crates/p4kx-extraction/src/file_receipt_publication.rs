// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};

use crate::origin_file_io::{self, StageDisposition};
use crate::output_file::StagedOutput;
use crate::{
    CancelToken, ConflictPolicy, OriginArtifactBinding, OriginArtifactDigest, OriginDigestWriter,
    OwnedFileToken, WriteProtectionError, owned_output_failure,
};

use super::lock::{Contention, ReceiptLock, protection_error, regular_file};
use super::{ExistingCompanions, FileReceipt};

#[cfg(test)]
#[path = "file_receipt_publication_tests.rs"]
mod tests;

/// The result of publishing already available artifact bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FilePublication {
    /// The artifact and its receipt were published.
    Written {
        /// The written byte length.
        size: u64,
    },
    /// An existing artifact and its receipt were retained.
    Kept {
        /// The observed existing byte length.
        size: u64,
        /// The retained bytes were compared in full with the supplied bytes.
        verified_content: bool,
    },
}

impl FileReceipt<'_> {
    /// Identifies cancellation from a publication error before the artifact commits.
    ///
    /// A retained output or failed lock retirement takes precedence over cancellation.
    #[must_use]
    pub fn is_precommit_cancellation(error: &io::Error, cancel: &CancelToken) -> bool {
        if super::retained_lock_failure(error) {
            return false;
        }
        if let Some(cause) = owned_output_failure(error) {
            return cause.is_cancelled();
        }
        if let Some(cause) = error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<WriteProtectionError>())
        {
            return cause.is_cancelled();
        }
        cancel.is_cancelled() && error.kind() == io::ErrorKind::Interrupted
    }

    pub(super) fn publish_owned_companion(
        &self,
        held: &ReceiptLock<'_>,
        digest: OriginArtifactDigest,
        owner: &OwnedFileToken,
        checkpoint: impl FnOnce() -> io::Result<()>,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> io::Result<()> {
        let binding = OriginArtifactBinding::File {
            path: self.artifact_name.clone(),
            digest,
        };
        origin_file_io::write_replacing_before(
            &self.receipt,
            self.receipt_protection,
            Some(&self.cancel),
            |writer| describe(writer, &binding),
            |_| {
                held.check(&self.cancel)?;
                checkpoint()?;
                owner
                    .ensure_renamed_writable(&self.artifact, self.cancel.flag())
                    .map_err(protection_error)
            },
        )
    }

    /// Publishes byte content and its receipt according to the conflict policy.
    /// Calls `describe` only for a committed artifact replacement.
    ///
    /// # Errors
    /// Returns a read, write, cancellation, ownership or receipt failure.
    pub fn publish_bytes(
        &self,
        bytes: &[u8],
        policy: ConflictPolicy,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> io::Result<FilePublication> {
        check_cancelled(&self.cancel)?;
        let held = self.acquire(Contention::Refuse)?;
        let mut committed = false;
        let result = (|| {
            self.check_target()?;
            if let Some(kept) = self.retained(&held, bytes, policy)? {
                return Ok(kept);
            }
            let (digest, owner) = origin_file_io::write_staged_retaining_owner(
                &self.artifact,
                self.protection,
                Some(&self.cancel),
                |writer| {
                    let mut measured = OriginDigestWriter::new(writer);
                    measured.write_all(bytes)?;
                    measured.finish()
                },
                |digest, staged| self.place_bytes(&held, staged, policy, digest),
            )?;
            let Some(digest) = digest else {
                return self
                    .retained(&held, bytes, ConflictPolicy::Skip)?
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::NotFound, "the retained target disappeared")
                    });
            };
            committed = true;
            let size = digest.byte_length();
            self.publish_owned_companion(&held, digest, &owner, || Ok(()), describe)?;
            Ok(FilePublication::Written { size })
        })();
        held.finish_io(result, committed)
    }

    fn retained(
        &self,
        held: &ReceiptLock<'_>,
        bytes: &[u8],
        policy: ConflictPolicy,
    ) -> io::Result<Option<FilePublication>> {
        held.check(&self.cancel)?;
        if policy == ConflictPolicy::Overwrite {
            return Ok(None);
        }
        let Some(metadata) = regular_file(&self.artifact)? else {
            return Ok(None);
        };
        let size = metadata.len();
        if policy == ConflictPolicy::Skip {
            return Ok(Some(FilePublication::Kept {
                size,
                verified_content: false,
            }));
        }
        if size != bytes.len() as u64 {
            return Ok(None);
        }
        let matches = matches_bytes(File::open(&self.artifact)?, bytes, &self.cancel)?;
        held.check(&self.cancel)?;
        Ok(matches.then_some(FilePublication::Kept {
            size,
            verified_content: true,
        }))
    }

    fn place_bytes(
        &self,
        held: &ReceiptLock<'_>,
        staged: &StagedOutput<'_>,
        policy: ConflictPolicy,
        digest: OriginArtifactDigest,
    ) -> io::Result<StageDisposition<Option<OriginArtifactDigest>>> {
        held.check(&self.cancel)?;
        self.check_target()?;
        if policy == ConflictPolicy::Skip && regular_file(&self.artifact)?.is_some() {
            return Ok(StageDisposition::Cleanup(None));
        }
        self.invalidate(ExistingCompanions::RequireOwned)?;
        staged.check().map_err(protection_error)?;
        held.check(&self.cancel)?;
        if policy != ConflictPolicy::Skip {
            fs::rename(staged.temporary(), &self.artifact)?;
            return Ok(StageDisposition::Moved(Some(digest)));
        }
        let installed = origin_file_io::install_without_replacing(staged, &self.artifact)?;
        Ok(StageDisposition::Cleanup(installed.then_some(digest)))
    }
}

fn matches_bytes(file: File, mut expected: &[u8], cancel: &CancelToken) -> io::Result<bool> {
    let mut retained = BufReader::new(file);
    loop {
        check_cancelled(cancel)?;
        let bytes = retained.fill_buf()?;
        if bytes.is_empty() {
            return Ok(expected.is_empty());
        }
        let Some(remaining) = expected.strip_prefix(bytes) else {
            return Ok(false);
        };
        let checked = bytes.len();
        retained.consume(checked);
        expected = remaining;
    }
}

fn check_cancelled(cancel: &CancelToken) -> io::Result<()> {
    if cancel.is_cancelled() {
        return Err(io::Error::from(io::ErrorKind::Interrupted));
    }
    Ok(())
}
