// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, OriginArtifactBinding, OriginArtifactName,
    OriginDigestWriter, check_protected_output,
};

use crate::query_file;

#[path = "file_receipt_header.rs"]
mod header;
#[path = "file_receipt_lock.rs"]
mod lock;

#[cfg(test)]
#[path = "file_receipt_tests.rs"]
mod tests;

use header::ReceiptOwner;
use lock::{ReceiptLock, invalid, protection_error, regular_file};

const COMPANION_SUFFIX: &str = ".p4kx-origin.jsonl";

/// Bound file paths and guard policy for one ordinary artifact publication.
pub(crate) struct FileReceipt<'source> {
    artifact: PathBuf,
    receipt: PathBuf,
    artifact_name: OriginArtifactName,
    protection: Option<&'source ArchiveWriteProtection>,
    receipt_protection: Option<&'source ArchiveWriteProtection>,
    cancel: CancelToken,
}

impl<'source> FileReceipt<'source> {
    /// Binds the output and its canonical companion in the same physical directory.
    pub(crate) fn new(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        receipt_protection: Option<&'source ArchiveWriteProtection>,
    ) -> io::Result<Self> {
        let cancel = CancelToken::new();
        let artifact = target(output, protection, &cancel)?;
        let name = artifact
            .file_name()
            .ok_or_else(|| invalid("the output names no file"))?;
        let mut receipt_name = name.to_os_string();
        receipt_name.push(COMPANION_SUFFIX);
        let receipt = artifact.with_file_name(receipt_name);
        check(&receipt, receipt_protection, &cancel)?;
        regular_file(&receipt)?;
        let artifact_name = OriginArtifactName::from_basename(name)?;
        Ok(Self {
            artifact,
            receipt,
            artifact_name,
            protection,
            receipt_protection,
            cancel,
        })
    }

    /// Returns the canonical receipt destination for the summary.
    pub(crate) fn path(&self) -> &Path {
        &self.receipt
    }

    /// Publishes the artifact and then its receipt under one guarded directory lock.
    pub(crate) fn publish<T>(
        &self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<T>,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding, &T) -> io::Result<()>,
    ) -> io::Result<T> {
        let mut held = None;
        let (result, digest) = query_file::write_replacing_before(
            &self.artifact,
            self.protection,
            Some(&self.cancel),
            |writer| {
                let mut measured = OriginDigestWriter::new(writer);
                let result = write(&mut measured)?;
                Ok((result, measured.finish()?))
            },
            || {
                let lock = self.acquire()?;
                self.invalidate()?;
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
        query_file::write_replacing_before(
            &self.receipt,
            self.receipt_protection,
            Some(&self.cancel),
            |writer| describe(writer, &binding, &result),
            || held.check(&self.cancel),
        )?;
        Ok(result)
    }

    fn acquire(&self) -> io::Result<ReceiptLock> {
        let directory = self
            .artifact
            .parent()
            .ok_or_else(|| invalid("the output has no directory"))?;
        let held = ReceiptLock::acquire(
            directory,
            self.receipt_protection.or(self.protection),
            &self.cancel,
        )?;
        let actual = target(&self.artifact, self.protection, &self.cancel)?;
        if actual != self.artifact {
            return Err(invalid("the artifact spelling changed before publication"));
        }
        Ok(held)
    }

    fn invalidate(&self) -> io::Result<()> {
        check(&self.receipt, self.receipt_protection, &self.cancel)?;
        if header::check_existing(&self.receipt, ReceiptOwner::File(&self.artifact_name))? {
            fs::remove_file(&self.receipt)?;
        }
        Ok(())
    }
}

/// A guarded destination for an explicit durable stdout receipt.
pub(crate) struct StdoutReceipt<'source> {
    path: PathBuf,
    protection: Option<&'source ArchiveWriteProtection>,
    cancel: CancelToken,
}

impl<'source> StdoutReceipt<'source> {
    /// Binds a durable receipt path before the raw stream is written.
    pub(crate) fn new(
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
    pub(crate) fn publish(
        &self,
        describe: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut held = None;
        let result = query_file::write_replacing_before(
            &self.path,
            self.protection,
            Some(&self.cancel),
            |writer| describe(writer),
            || {
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
    let name = path
        .file_name()
        .ok_or_else(|| invalid("the receipt names no file"))?;
    let mut companion_name = name.to_os_string();
    companion_name.push(COMPANION_SUFFIX);
    if regular_file(&path.with_file_name(companion_name))?.is_some() {
        return Err(invalid("the receipt target has its own artifact companion"));
    }
    header::check_existing(path, ReceiptOwner::Stdout)?;
    Ok(())
}

fn target(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
) -> io::Result<PathBuf> {
    check(path, protection, cancel)?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("the output names no file"))?;
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent)?;
    let target = parent.join(name);
    let actual = if regular_file(&target)?.is_some() {
        fs::canonicalize(target)?
    } else {
        target
    };
    let name = actual
        .file_name()
        .ok_or_else(|| invalid("the output names no file"))?;
    if name
        .to_str()
        .is_some_and(|name| name.eq_ignore_ascii_case(lock::LOCK_NAME))
    {
        return Err(invalid("the output names the origin publication lock"));
    }
    if name
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(COMPANION_SUFFIX)
    {
        return Err(invalid("the output names a reserved origin companion"));
    }
    #[cfg(windows)]
    if name.to_string_lossy().contains(':') {
        return Err(invalid("the origin target names an alternate data stream"));
    }
    check(&actual, protection, cancel)?;
    Ok(actual)
}

fn check(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
) -> io::Result<()> {
    check_protected_output(path, protection, Some(cancel)).map_err(protection_error)
}
