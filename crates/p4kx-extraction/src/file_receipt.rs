// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{
    ArchiveWriteProtection, CancelToken, OriginArtifactBinding, OriginArtifactName,
    OriginDigestWriter, check_protected_output,
};

use crate::origin_file_io;

#[path = "file_receipt_directory.rs"]
mod directory;
#[path = "file_receipt_directory_table.rs"]
mod directory_table;
#[path = "origin_directory_wire.rs"]
mod directory_wire;
#[path = "file_receipt_header.rs"]
mod header;
#[path = "file_receipt_lock.rs"]
mod lock;
#[path = "file_receipt_lock_retirement.rs"]
mod lock_retirement;
#[path = "file_receipt_ordinary.rs"]
mod ordinary;
#[path = "file_receipt_target.rs"]
mod target_path;

#[cfg(test)]
#[path = "file_receipt_ordinary_tests.rs"]
mod ordinary_tests;
#[cfg(test)]
#[path = "file_receipt_tests.rs"]
mod tests;

pub use directory::{DirectoryCommit, DirectoryReceipt};
use directory_table::ReplacementPath;
use header::ReceiptOwner;
use lock::{Contention, ReceiptLock, invalid, protection_error, regular_file};
pub(crate) use ordinary::OrdinaryReplacement;
use target_path::{BindTarget, TargetLeaf, bind_target, lexical_receipt, sibling_receipt, target};

const COMPANION_SUFFIX: &str = ".p4kx-origin.jsonl";
const DIRECTORY_NAME: &str = COMPANION_SUFFIX;

/// Bound file paths and guard policy for one ordinary artifact publication.
pub struct FileReceipt<'source> {
    artifact: PathBuf,
    leaf: TargetLeaf,
    receipt: PathBuf,
    artifact_name: OriginArtifactName,
    protection: Option<&'source ArchiveWriteProtection>,
    receipt_protection: Option<&'source ArchiveWriteProtection>,
    cancel: CancelToken,
}

impl<'source> FileReceipt<'source> {
    /// Binds the output and its canonical companion in the same physical directory.
    ///
    /// # Errors
    ///
    /// Returns path, target ownership or output protection errors.
    pub fn new(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        receipt_protection: Option<&'source ArchiveWriteProtection>,
    ) -> io::Result<Self> {
        Self::new_with_cancel(output, protection, receipt_protection, CancelToken::new())
    }

    /// Binds the output and canonical companion to the supplied cancellation token.
    ///
    /// # Errors
    ///
    /// Returns path, target ownership or output protection errors.
    pub fn new_with_cancel(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        receipt_protection: Option<&'source ArchiveWriteProtection>,
        cancel: CancelToken,
    ) -> io::Result<Self> {
        let binding = Self::bind(
            output,
            protection,
            receipt_protection,
            cancel,
            BindTarget::Receipt,
        )?;
        check(
            &binding.receipt,
            binding.receipt_protection,
            &binding.cancel,
        )?;
        regular_file(&binding.receipt)?;
        Ok(binding)
    }

    fn bind(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        receipt_protection: Option<&'source ArchiveWriteProtection>,
        cancel: CancelToken,
        mode: BindTarget,
    ) -> io::Result<Self> {
        let bound = bind_target(output, protection, &cancel, mode)?;
        let artifact = bound.path;
        let name = artifact
            .file_name()
            .ok_or_else(|| invalid("the output names no file"))?;
        let receipt = sibling_receipt(&artifact)?;
        let artifact_name = OriginArtifactName::from_basename(name)?;
        Ok(Self {
            artifact,
            leaf: bound.leaf,
            receipt,
            artifact_name,
            protection,
            receipt_protection,
            cancel,
        })
    }

    /// Returns the canonical receipt destination for the summary.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.receipt
    }

    /// Publishes the artifact and then its receipt under one guarded directory lock.
    ///
    /// # Errors
    ///
    /// Returns a write, output protection, ownership or receipt failure.
    pub fn publish<T>(
        &self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<T>,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding, &T) -> io::Result<()>,
    ) -> io::Result<T> {
        let mut held = None;
        let (result, digest) = origin_file_io::write_replacing_before(
            &self.artifact,
            self.protection,
            Some(&self.cancel),
            |writer| {
                let mut measured = OriginDigestWriter::new(writer);
                let result = write(&mut measured)?;
                Ok((result, measured.finish()?))
            },
            |staged| {
                let lock = self.acquire(Contention::Refuse)?;
                self.check_target()?;
                staged.check().map_err(protection_error)?;
                self.invalidate(ExistingCompanions::RequireOwned)?;
                held = Some(lock);
                Ok(())
            },
        )?;
        let held =
            held.ok_or_else(|| io::Error::other("artifact committed without the origin lock"))?;
        let binding = OriginArtifactBinding::File {
            path: self.artifact_name.clone(),
            digest,
        };
        origin_file_io::write_replacing_before(
            &self.receipt,
            self.receipt_protection,
            Some(&self.cancel),
            |writer| describe(writer, &binding, &result),
            |_| held.check(&self.cancel),
        )?;
        Ok(result)
    }

    fn acquire(&self, contention: Contention) -> io::Result<ReceiptLock> {
        let directory = self
            .artifact
            .parent()
            .ok_or_else(|| invalid("the output has no directory"))?;
        ReceiptLock::acquire_with(
            directory,
            self.receipt_protection.or(self.protection),
            &self.cancel,
            contention,
        )
    }

    fn check_target(&self) -> io::Result<()> {
        let mode = match self.leaf {
            TargetLeaf::File => BindTarget::Receipt,
            TargetLeaf::SymbolicLink => BindTarget::Replacement,
        };
        let actual = bind_target(&self.artifact, self.protection, &self.cancel, mode)?;
        if actual.path != self.artifact {
            return Err(invalid("the artifact spelling changed before publication"));
        }
        if actual.leaf != self.leaf {
            return Err(invalid("the artifact type changed before publication"));
        }
        Ok(())
    }

    fn invalidate(&self, existing: ExistingCompanions) -> io::Result<()> {
        let aggregate = self.artifact.with_file_name(DIRECTORY_NAME);
        if existing == ExistingCompanions::RequireOwned {
            check(&self.receipt, self.receipt_protection, &self.cancel)?;
            check(&aggregate, self.receipt_protection, &self.cancel)?;
        }
        let lexical = if self.leaf == TargetLeaf::SymbolicLink {
            lexical_receipt(&self.receipt)?
        } else {
            None
        };
        let previous = match (&self.leaf, &lexical) {
            (TargetLeaf::File, _) => Some((self.receipt.as_path(), &self.artifact_name)),
            (TargetLeaf::SymbolicLink, Some(owner)) => Some((self.receipt.as_path(), owner)),
            (TargetLeaf::SymbolicLink, None) => None,
        };
        let previous = previous.map_or(Ok(None), |(path, owner)| {
            existing
                .owned(header::check_existing(path, ReceiptOwner::File(owner)))
                .map(|owned| owned.then_some(path))
        })?;
        let replacement = match self.leaf {
            TargetLeaf::File => ReplacementPath::File(&self.artifact),
            TargetLeaf::SymbolicLink => ReplacementPath::SymbolicLink(&self.artifact),
        };
        let aggregate_owned = existing.owned(
            directory_table::lookup(&aggregate, Some(replacement))
                .map(|overlap| overlap == Some(true)),
        )?;
        if existing == ExistingCompanions::PreserveForeign {
            if aggregate_owned {
                check(&aggregate, self.receipt_protection, &self.cancel)?;
            }
            if let Some(previous) = previous {
                check(previous, self.receipt_protection, &self.cancel)?;
            }
        }
        if aggregate_owned {
            fs::remove_file(&aggregate)?;
        }
        if let Some(previous) = previous {
            fs::remove_file(previous)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExistingCompanions {
    RequireOwned,
    PreserveForeign,
}

impl ExistingCompanions {
    fn owned(self, observed: io::Result<bool>) -> io::Result<bool> {
        match observed {
            Err(error)
                if self == Self::PreserveForeign
                    && error.kind() == io::ErrorKind::InvalidInput
                    && error.raw_os_error().is_none() =>
            {
                Ok(false)
            }
            result => result,
        }
    }
}

/// A guarded destination for an explicit durable stdout receipt.
pub struct StdoutReceipt<'source> {
    path: PathBuf,
    protection: Option<&'source ArchiveWriteProtection>,
    cancel: CancelToken,
}

impl<'source> StdoutReceipt<'source> {
    /// Binds a durable receipt path before the raw stream is written.
    ///
    /// # Errors
    ///
    /// Returns path, target ownership or output protection errors.
    pub fn new(
        path: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
    ) -> io::Result<Self> {
        let cancel = CancelToken::new();
        let path = target(path, protection, &cancel)?;
        check_stdout_target(&path)?;
        Ok(Self {
            path,
            protection,
            cancel,
        })
    }

    /// Publishes a completed receipt without changing the raw stdout bytes.
    ///
    /// # Errors
    ///
    /// Returns a write, output protection or ownership error.
    pub fn publish(
        &self,
        describe: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut held = None;
        let result = origin_file_io::write_replacing_before(
            &self.path,
            self.protection,
            Some(&self.cancel),
            |writer| describe(writer),
            |_| {
                let directory = self
                    .path
                    .parent()
                    .ok_or_else(|| invalid("the receipt has no directory"))?;
                let lock = ReceiptLock::acquire(directory, self.protection, &self.cancel)?;
                if target(&self.path, self.protection, &self.cancel)? != self.path {
                    return Err(invalid("the receipt spelling changed before publication"));
                }
                check_stdout_target(&self.path)?;
                held = Some(lock);
                Ok(())
            },
        );
        drop(held);
        result
    }
}

fn check_stdout_target(path: &Path) -> io::Result<()> {
    if regular_file(&sibling_receipt(path)?)?.is_some() {
        return Err(invalid("the receipt target has its own artifact companion"));
    }
    header::check_existing(path, ReceiptOwner::Stdout)?;
    Ok(())
}

fn check(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
) -> io::Result<()> {
    check_protected_output(path, protection, Some(cancel)).map_err(protection_error)
}
