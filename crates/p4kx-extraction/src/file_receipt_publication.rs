// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};

use crate::origin_file_io::{self, StageDisposition};
use crate::output_file::StagedOutput;
use crate::{
    CancelToken, ConflictPolicy, OriginArtifactBinding, OriginArtifactDigest, OriginDigestWriter,
    OwnedFileToken,
};

use super::lock::{Contention, ReceiptLock, invalid, protection_error, regular_file};
use super::target_path::{BindTarget, bind_target};
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
    pub(super) fn publish_companion(
        &self,
        held: &ReceiptLock<'_>,
        digest: OriginArtifactDigest,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> io::Result<()> {
        self.publish_companion_checked(held, digest, || self.check_committed_target(), describe)
    }

    pub(super) fn publish_owned_companion(
        &self,
        held: &ReceiptLock<'_>,
        digest: OriginArtifactDigest,
        owner: &OwnedFileToken,
        checkpoint: impl FnOnce() -> io::Result<()>,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> io::Result<()> {
        self.publish_companion_checked(
            held,
            digest,
            || {
                checkpoint()?;
                owner
                    .ensure_renamed_writable(&self.artifact, self.cancel.flag())
                    .map_err(protection_error)
            },
            describe,
        )
    }

    fn publish_companion_checked(
        &self,
        held: &ReceiptLock<'_>,
        digest: OriginArtifactDigest,
        check_committed: impl FnOnce() -> io::Result<()>,
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
                check_committed()
            },
        )
    }

    fn check_committed_target(&self) -> io::Result<()> {
        let mode = if self.native_entry {
            self.replacement_mode()
        } else {
            BindTarget::Receipt
        };
        let actual = bind_target(&self.artifact, self.protection, &self.cancel, mode)?.path;
        if actual != self.artifact || regular_file(&actual)?.is_none() {
            return Err(invalid(
                "the committed artifact is no longer a regular file at its output path",
            ));
        }
        Ok(())
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
