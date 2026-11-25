// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{Chunk, Destination, EntryWriter, Outcome, write_error};
use crate::file_receipt::{
    self, FreshPublication, StagedReceiptPublication, finish_staged_receipt, try_create_fresh,
};
use crate::origin_file_io::StageDisposition;
use crate::output_file::StagedOutput;
use crate::source::{SourceEntry, SourceReader};
use crate::{
    ConflictPolicy, ExtractionError, FieldOrigin, FileReceipt, InputScope, OriginArtifactDigest,
    OriginDigestWriter, OriginOutputAddress, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceUseRole, SourceUses, write_origin_companion,
};

/// An unchanged file installed by Extraction.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct RawEntryArtifact {
    /// The output file relative to the extraction target.
    pub path: String,
    /// The measured byte length and SHA-256 of the output.
    pub digest: OriginArtifactDigest,
    /// The relative source companion, when its publication succeeded.
    pub origin_receipt: Option<String>,
}

impl EntryWriter<'_> {
    pub(super) fn copy_with_receipt(
        &self,
        entry: &SourceEntry<'_>,
        reader: &mut SourceReader<'_>,
        destination: &Destination,
        prefix: &[u8],
        buffer: &mut [u8],
        written: &mut u64,
    ) -> Result<Outcome, ExtractionError> {
        let paths = RawPaths::prepare(self, entry, &destination.file)?;
        let archive = entry.observed_entry();
        let (temporary, file, ownership) = self.temporary(&destination.folder)?;
        let mut output = OriginDigestWriter::new(file);
        let copied = output
            .write_all(prefix)
            .map_err(|error| write_error(destination.file.clone(), error))
            .and_then(|()| self.copy(reader, &mut output, buffer, &destination.file, written));
        let staged = StagedOutput::new(
            &temporary,
            &destination.file,
            &ownership,
            self.protection,
            self.cancel,
        );
        let result = match copied {
            Ok(Chunk::End) => output
                .finish()
                .map_err(|error| write_error(destination.file.clone(), error))
                .and_then(|digest| {
                    let file = reader.file_origin();
                    let input = archive.map_or_else(
                        || {
                            file.as_ref()
                                .map_or(SourceInput::Unknown, SourceInput::File)
                        },
                        SourceInput::ArchiveView,
                    );
                    self.publish_raw(&staged, paths, digest, input, entry)
                }),
            Ok(_) => {
                drop(output);
                Ok(StageDisposition::Cleanup(Outcome::Cancelled))
            }
            Err(error) => {
                drop(output);
                Err(error)
            }
        };
        match result {
            Ok(StageDisposition::Moved(outcome)) => Ok(outcome),
            result => crate::output_cleanup::extraction(
                result.map(|stage| match stage {
                    StageDisposition::Cleanup(outcome) | StageDisposition::Moved(outcome) => {
                        outcome
                    }
                }),
                &ownership,
                &temporary,
                self.cancel,
            ),
        }
    }

    pub(crate) fn publish_observed_raw(
        &self,
        staged: &StagedOutput<'_>,
        digest: OriginArtifactDigest,
        input: SourceInput<'_>,
        entry: &SourceEntry<'_>,
    ) -> Result<StageDisposition<Outcome>, ExtractionError> {
        let paths = RawPaths::prepare(self, entry, staged.output())?;
        self.publish_raw(staged, paths, digest, input, entry)
    }

    fn publish_raw(
        &self,
        staged: &StagedOutput<'_>,
        paths: RawPaths,
        digest: OriginArtifactDigest,
        input: SourceInput<'_>,
        entry: &SourceEntry<'_>,
    ) -> Result<StageDisposition<Outcome>, ExtractionError> {
        let output = staged.output();
        let receipt = match FileReceipt::for_native_staged_replacement(
            output,
            self.protection,
            self.cancel.cloned().unwrap_or_default(),
        ) {
            Ok(receipt) => receipt,
            Err(error) => return self.recover_uncommitted_raw(staged, paths, digest, error),
        };
        let evidence = RawOrigins::capture(input, digest.byte_length())
            .map_err(|error| write_error(paths.receipt.clone(), io::Error::other(error)))?;
        let result = receipt.publish_staged_with_policy(
            staged,
            digest.clone(),
            self.policy,
            || self.receipt_checkpoint(),
            || self.tracker.artifacts.invalidate(output),
            |writer, binding| {
                write_origin_companion(
                    writer,
                    binding,
                    &evidence.inventory,
                    [(&OriginOutputAddress::OpaquePayload, &evidence.origin)],
                )
            },
        );
        let (receipt, retirement) = match result {
            None => return Ok(StageDisposition::Cleanup(Outcome::Kept)),
            Some(StagedReceiptPublication::Uncommitted(error)) => {
                return self.recover_uncommitted_raw(staged, paths, digest, error);
            }
            Some(StagedReceiptPublication::Committed {
                receipt,
                retirement,
            }) => (receipt, retirement),
        };
        let available = receipt.as_ref().is_some_and(Result::is_ok);
        if available && paths.owns_name {
            self.tracker.written.record(&paths.companion, entry.index());
        }
        let issue = finish_staged_receipt(receipt, retirement)
            .err()
            .map(|error| write_error(paths.receipt.clone(), error));
        self.tracker
            .artifacts
            .insert_raw(output, paths.artifact(digest, available));
        let outcome = Outcome::RawWritten(issue);
        Ok(if self.policy == ConflictPolicy::Skip {
            StageDisposition::Cleanup(outcome)
        } else {
            StageDisposition::Moved(outcome)
        })
    }

    fn recover_uncommitted_raw(
        &self,
        staged: &StagedOutput<'_>,
        paths: RawPaths,
        digest: OriginArtifactDigest,
        error: io::Error,
    ) -> Result<StageDisposition<Outcome>, ExtractionError> {
        if error.kind() == io::ErrorKind::Interrupted && self.tracker.should_stop() {
            return Ok(StageDisposition::Cleanup(Outcome::Cancelled));
        }
        let output = staged.output();
        let Some(lock) = file_receipt::origin_lock_collision(&error) else {
            return Err(write_error(output.to_path_buf(), error));
        };
        let outcome = match try_create_fresh(staged, self.cancel, || self.tracker.should_stop())? {
            FreshPublication::Installed => {
                self.tracker
                    .artifacts
                    .insert_raw(output, paths.artifact(digest, false));
                Outcome::Written
            }
            FreshPublication::Cancelled => Outcome::Cancelled,
            FreshPublication::Contended if self.policy == ConflictPolicy::Skip => Outcome::Kept,
            FreshPublication::Contended | FreshPublication::RequiresLock => {
                return Err(ExtractionError::OriginLockCollision {
                    path: lock.to_path_buf(),
                });
            }
        };
        Ok(StageDisposition::Cleanup(outcome))
    }
}

struct RawPaths {
    relative: String,
    relative_receipt: String,
    companion: PathBuf,
    receipt: PathBuf,
    owns_name: bool,
}

impl RawPaths {
    fn prepare(
        writer: &EntryWriter<'_>,
        entry: &SourceEntry<'_>,
        output: &Path,
    ) -> Result<Self, ExtractionError> {
        let relative = output
            .strip_prefix(writer.target)
            .map_err(|_| ExtractionError::UnsafePath)?;
        let companion = file_receipt::companion_path(relative)
            .map_err(|error| write_error(output.to_path_buf(), error))?;
        let receipt = writer.target.join(&companion);
        writer.case_collision(&companion, entry)?;
        crate::portable_name::check_path_length(&receipt)?;
        Ok(Self {
            relative: report_path(relative)?,
            relative_receipt: report_path(&companion)?,
            companion,
            receipt,
            owns_name: relative == writer.relative_path(entry)?.as_path(),
        })
    }

    fn artifact(self, digest: OriginArtifactDigest, available: bool) -> RawEntryArtifact {
        RawEntryArtifact {
            path: self.relative,
            digest,
            origin_receipt: available.then_some(self.relative_receipt),
        }
    }
}

fn report_path(path: &Path) -> Result<String, ExtractionError> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or(ExtractionError::UnsafePath)
}

struct RawOrigins {
    inventory: SourceInventory,
    origin: FieldOrigin,
}

impl RawOrigins {
    fn capture(input: SourceInput<'_>, length: u64) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let origin = if matches!(input, SourceInput::Unknown) {
            uses.origin(&[], false)
        } else {
            let namespace = if matches!(input, SourceInput::ArchiveView(_)) {
                "archiveEntryDecodedBytes"
            } else {
                "standaloneFileBytes"
            };
            let scope = uses.observe(input, InputScope::Payload)?;
            let actual = scope.at(
                SourceLocation::Subresource {
                    namespace: namespace.to_owned(),
                    indices: vec![0, length],
                    field: "bytes".to_owned(),
                },
                SourceUseRole::Payload,
            )?;
            uses.origin(&[actual], true)
        };
        Ok(Self {
            inventory: uses.finish_with(|| Ok::<(), SourceEvidenceError>(()))?,
            origin,
        })
    }
}
