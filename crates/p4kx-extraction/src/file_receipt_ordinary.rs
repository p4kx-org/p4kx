// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use super::lock::{Contention, ReceiptLock};
use super::target_path::BindTarget;
use super::{ExistingCompanions, FileReceipt};
use crate::output_file::write_error;
use crate::{ArchiveWriteProtection, CancelToken, ConversionError};

pub(crate) struct OrdinaryReplacement<'source> {
    receipt: FileReceipt<'source>,
    held: ReceiptLock,
}

impl<'source> OrdinaryReplacement<'source> {
    pub(crate) fn new(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Self> {
        let receipt = FileReceipt::bind(
            output,
            protection,
            protection,
            cancel.cloned().unwrap_or_default(),
            BindTarget::Replacement,
        )?;
        let held = receipt.acquire(Contention::Wait)?;
        Ok(Self { receipt, held })
    }

    pub(crate) fn invalidate(&self) -> io::Result<()> {
        self.held.check(&self.receipt.cancel)?;
        self.receipt.check_target()?;
        self.receipt.invalidate(ExistingCompanions::PreserveForeign)
    }

    pub(crate) fn finish_conversion<T>(
        self,
        out: &Path,
        result: Result<T, ConversionError>,
    ) -> Result<T, ConversionError> {
        self.finish(result, |error| write_error(out, io::Error::other(error)))
    }

    pub(crate) fn finish_io<T>(self, result: io::Result<T>) -> io::Result<T> {
        self.finish(result, io::Error::other)
    }

    fn finish<T, E: std::error::Error + 'static>(
        self,
        result: Result<T, E>,
        map: impl FnOnce(LockFinishFailure<E>) -> E,
    ) -> Result<T, E> {
        let Err(cleanup) = self.held.retire(self.receipt.receipt_protection) else {
            return result;
        };
        Err(map(match result {
            Ok(_) => LockFinishFailure::Committed { cleanup },
            Err(primary) => LockFinishFailure::Uncommitted { primary, cleanup },
        }))
    }
}

#[derive(Debug, thiserror::Error)]
enum LockFinishFailure<E> {
    #[error("output committed but its temporary publication lock could not be retired")]
    Committed {
        #[source]
        cleanup: io::Error,
    },
    #[error(
        "output failed before commit and its temporary publication lock could not be retired: {cleanup}"
    )]
    Uncommitted {
        #[source]
        primary: E,
        cleanup: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;

    use super::{LockFinishFailure, OrdinaryReplacement};
    use crate::file_receipt::lock::LOCK_NAME;

    #[test]
    fn retirement_distinguishes_commit_status_and_preserves_foreign_lock() -> io::Result<()> {
        for committed in [false, true] {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("output.bin");
            let lock = dir.path().join(LOCK_NAME);
            let replaced = dir.path().join("original-lock");
            let replacement = OrdinaryReplacement::new(&path, None, None)?;
            fs::rename(&lock, &replaced)?;
            fs::write(&lock, b"foreign")?;
            let outcome = if committed {
                Ok(())
            } else {
                Err(io::Error::other("write failed before commit"))
            };
            let failed = replacement
                .finish_io(outcome)
                .expect_err("a foreign lock cannot be retired");
            let typed = failed
                .get_ref()
                .and_then(|error| error.downcast_ref::<LockFinishFailure<io::Error>>());
            match (committed, typed) {
                (true, Some(LockFinishFailure::Committed { .. })) => {}
                (false, Some(LockFinishFailure::Uncommitted { primary, .. })) => {
                    assert_eq!(primary.to_string(), "write failed before commit");
                }
                _ => panic!("wrong commit state for {failed:?}"),
            }
            assert_eq!(fs::read(&lock)?, b"foreign");
            assert_eq!(fs::read(&replaced)?, b"");
        }
        Ok(())
    }
}
