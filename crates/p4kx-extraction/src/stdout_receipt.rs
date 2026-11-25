// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{ArchiveWriteProtection, CancelToken, origin_file_io};

use super::header::{self, ReceiptOwner};
use super::lock::{ReceiptLock, invalid, regular_file};
use super::target_path::{sibling_receipt, target};

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
                let lease = held.insert(ReceiptLock::acquire(
                    directory,
                    self.protection,
                    &self.cancel,
                )?);
                if target(&self.path, self.protection, &self.cancel)? != self.path {
                    return Err(invalid("the receipt spelling changed before publication"));
                }
                check_stdout_target(&self.path)?;
                lease.check(&self.cancel)
            },
        );
        match held {
            Some(held) => {
                let committed = result.is_ok();
                held.finish_io(result, committed)
            }
            None => result,
        }
    }
}

fn check_stdout_target(path: &Path) -> io::Result<()> {
    if regular_file(&sibling_receipt(path)?)?.is_some() {
        return Err(invalid("the receipt target has its own artifact companion"));
    }
    header::check_existing(path, ReceiptOwner::Stdout)?;
    Ok(())
}
