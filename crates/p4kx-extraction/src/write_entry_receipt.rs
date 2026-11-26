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
use crate::portable_name;
use crate::source::{SourceEntry, SourceReader};
use crate::{
    ConflictPolicy, ExtractionError, FieldOrigin, FileReceipt, InputScope, OriginArtifactBinding,
    OriginArtifactDigest, OriginDigestWriter, OriginOutputAddress, SourceEvidenceError,
    SourceInput, SourceInventory, SourceUseRole, SourceUses, XmlReceipt, write_origin_companion,
};

/// A byte-bound file installed by Extraction and its portable companion.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct ExtractedFileArtifact {
    /// The output file relative to the extraction target.
    pub path: String,
    /// The measured byte length and SHA-256 of the output.
    pub digest: OriginArtifactDigest,
    /// The relative source companion, when its publication succeeded.
    pub origin_receipt: Option<String>,
}

/// An unchanged file installed by Extraction.
pub type RawEntryArtifact = ExtractedFileArtifact;

#[derive(Clone, Copy)]
enum ArtifactKind {
    Raw,
    CryXml,
}

impl ArtifactKind {
    fn insert(
        self,
        writer: &EntryWriter<'_>,
        output: &Path,
        artifact: ExtractedFileArtifact,
        issue: Option<ExtractionError>,
    ) -> Outcome {
        match self {
            Self::Raw => {
                writer.tracker.artifacts.insert_raw(output, artifact);
                Outcome::RawWritten(issue)
            }
            Self::CryXml => {
                writer.tracker.artifacts.insert_cryxml(output, artifact);
                Outcome::Converted(issue)
            }
        }
    }
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
        let paths = EntryReceiptPaths::prepare(self, entry, &destination.file, ArtifactKind::Raw)?;
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
        self.finish_staged_entry(result, &ownership, &temporary)
    }

    pub(crate) fn write_observed_cryxml(
        &self,
        entry: &SourceEntry<'_>,
        destination: &Destination,
        bytes: &[u8],
        evidence: &XmlReceipt,
    ) -> Result<Outcome, ExtractionError> {
        let paths =
            EntryReceiptPaths::prepare(self, entry, &destination.file, ArtifactKind::CryXml)?;
        let (temporary, file, ownership) = self.temporary(&destination.folder)?;
        let mut output = OriginDigestWriter::new(file);
        let digest = match output.write_all(bytes).and_then(|()| output.finish()) {
            Ok(digest) => digest,
            Err(error) => {
                let error = write_error(destination.file.clone(), error);
                return self.finish_staged_entry(Err(error), &ownership, &temporary);
            }
        };
        if let Err(error) = paths.check_cryxml_receipt(self, entry) {
            let issue = write_error(paths.receipt.clone(), error);
            let outcome = self.put_in_place(
                &temporary,
                &ownership,
                &destination.file,
                Ok(Chunk::End),
                Outcome::Converted(Some(issue)),
            )?;
            if matches!(outcome, Outcome::Converted(_)) {
                self.tracker
                    .artifacts
                    .insert_cryxml(&destination.file, paths.artifact(digest, false));
            }
            return Ok(outcome);
        }
        let staged = StagedOutput::new(
            &temporary,
            &destination.file,
            &ownership,
            self.protection,
            self.cancel,
        );
        let result = self.publish_entry_receipt(
            &staged,
            paths,
            digest,
            entry,
            ArtifactKind::CryXml,
            |writer, binding| evidence.write(writer, binding),
        );
        self.finish_staged_entry(result, &ownership, &temporary)
    }

    fn finish_staged_entry(
        &self,
        result: Result<StageDisposition<Outcome>, ExtractionError>,
        ownership: &crate::OwnedFileToken,
        temporary: &Path,
    ) -> Result<Outcome, ExtractionError> {
        match result {
            Ok(StageDisposition::Moved(outcome)) => Ok(outcome),
            result => crate::output_cleanup::extraction(
                result.map(|stage| match stage {
                    StageDisposition::Cleanup(outcome) | StageDisposition::Moved(outcome) => {
                        outcome
                    }
                }),
                ownership,
                temporary,
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
        let paths = EntryReceiptPaths::prepare(self, entry, staged.output(), ArtifactKind::Raw)?;
        self.publish_raw(staged, paths, digest, input, entry)
    }

    fn publish_raw(
        &self,
        staged: &StagedOutput<'_>,
        paths: EntryReceiptPaths,
        digest: OriginArtifactDigest,
        input: SourceInput<'_>,
        entry: &SourceEntry<'_>,
    ) -> Result<StageDisposition<Outcome>, ExtractionError> {
        let evidence = RawOrigins::capture(input, digest.byte_length())
            .map_err(|error| write_error(paths.receipt.clone(), io::Error::other(error)))?;
        self.publish_entry_receipt(
            staged,
            paths,
            digest,
            entry,
            ArtifactKind::Raw,
            |writer, binding| {
                write_origin_companion(
                    writer,
                    binding,
                    &evidence.inventory,
                    [(&OriginOutputAddress::OpaquePayload, &evidence.origin)],
                )
            },
        )
    }

    fn publish_entry_receipt(
        &self,
        staged: &StagedOutput<'_>,
        paths: EntryReceiptPaths,
        digest: OriginArtifactDigest,
        entry: &SourceEntry<'_>,
        kind: ArtifactKind,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> Result<StageDisposition<Outcome>, ExtractionError> {
        let output = staged.output();
        let receipt = match FileReceipt::for_native_staged_replacement(
            output,
            self.protection,
            self.cancel.cloned().unwrap_or_default(),
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                return self.recover_uncommitted_entry(staged, paths, digest, kind, error);
            }
        };
        let result = receipt.publish_staged_with_policy(
            staged,
            digest.clone(),
            self.policy,
            || self.receipt_checkpoint(),
            || self.tracker.artifacts.invalidate(output),
            describe,
        );
        let (receipt, retirement) = match result {
            None => return Ok(StageDisposition::Cleanup(Outcome::Kept)),
            Some(StagedReceiptPublication::Uncommitted(error)) => {
                return self.recover_uncommitted_entry(staged, paths, digest, kind, error);
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
        let outcome = kind.insert(self, output, paths.artifact(digest, available), issue);
        Ok(if self.policy == ConflictPolicy::Skip {
            StageDisposition::Cleanup(outcome)
        } else {
            StageDisposition::Moved(outcome)
        })
    }

    fn recover_uncommitted_entry(
        &self,
        staged: &StagedOutput<'_>,
        paths: EntryReceiptPaths,
        digest: OriginArtifactDigest,
        kind: ArtifactKind,
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
                kind.insert(self, output, paths.artifact(digest, false), None)
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

struct EntryReceiptPaths {
    relative: String,
    relative_receipt: String,
    companion: PathBuf,
    receipt: PathBuf,
    owns_name: bool,
}

impl EntryReceiptPaths {
    fn prepare(
        writer: &EntryWriter<'_>,
        entry: &SourceEntry<'_>,
        output: &Path,
        kind: ArtifactKind,
    ) -> Result<Self, ExtractionError> {
        let relative = output
            .strip_prefix(writer.target)
            .map_err(|_| ExtractionError::UnsafePath)?;
        let companion = file_receipt::companion_path(relative)
            .map_err(|error| write_error(output.to_path_buf(), error))?;
        let receipt = writer.target.join(&companion);
        if matches!(kind, ArtifactKind::Raw) {
            writer.case_collision(&companion, entry)?;
            portable_name::check_path_length(&receipt)?;
        }
        Ok(Self {
            relative: report_path(relative)?,
            relative_receipt: report_path(&companion)?,
            companion,
            receipt,
            owns_name: relative == writer.relative_path(entry)?.as_path(),
        })
    }

    fn check_cryxml_receipt(
        &self,
        writer: &EntryWriter<'_>,
        entry: &SourceEntry<'_>,
    ) -> io::Result<()> {
        writer
            .cryxml_companion_collision(&self.companion, entry)
            .map_err(io::Error::other)?;
        portable_name::check_path_length(&self.receipt).map_err(io::Error::other)
    }

    fn artifact(self, digest: OriginArtifactDigest, available: bool) -> ExtractedFileArtifact {
        ExtractedFileArtifact {
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
            let scope = uses.observe(input, InputScope::Payload)?;
            let actual = scope.at(
                crate::source_scope::raw_payload_location(input, length),
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
