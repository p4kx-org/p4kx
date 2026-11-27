// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use super::lock::{Contention, ReceiptLock, RetainedLock};
use super::staged::RetirementFailure;
use super::target_path::{BindTarget, is_companion_name};
use super::{DIRECTORY_NAME, ExistingCompanions, FileReceipt, ReceiptOwner, header};
use crate::output_file::write_error;
use crate::owned_output_failure::io_error_chain;
use crate::write::Outcome;
use crate::{
    ArchiveWriteProtection, CancelToken, ConversionError, ExtractionError, OriginArtifactBinding,
    OriginArtifactDigest, OwnedFileToken,
};

/// Whether the publication could not release its owned lock.
pub(crate) fn retained_lock_failure(error: &io::Error) -> bool {
    io_error_chain(error).any(|cause| {
        cause.is::<RetainedLock>()
            || cause.is::<RetirementFailure>()
            || cause.is::<LockFinishFailure<io::Error>>()
            || cause.is::<LockFinishFailure<ConversionError>>()
            || cause.is::<LockFinishFailure<ExtractionError>>()
    })
}

pub(crate) struct OrdinaryReplacement<'source> {
    receipt: FileReceipt<'source>,
    held: ReceiptLock<'source>,
    mode: BindTarget,
}

impl<'source> OrdinaryReplacement<'source> {
    pub(crate) fn new(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Self> {
        Self::bind(output, protection, cancel, BindTarget::Replacement)
    }

    pub(crate) fn for_entry(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Self> {
        Self::bind(output, protection, cancel, BindTarget::NativeReplacement)
    }

    fn bind(
        output: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: Option<&CancelToken>,
        mode: BindTarget,
    ) -> io::Result<Self> {
        let receipt = FileReceipt::bind(
            output,
            protection,
            protection,
            cancel.cloned().unwrap_or_default(),
            mode,
        )?;
        let held = receipt.acquire(Contention::Wait)?;
        Ok(Self {
            receipt,
            held,
            mode,
        })
    }

    pub(crate) fn invalidate(&self) -> io::Result<()> {
        self.held.check(&self.receipt.cancel)?;
        if self.mode == BindTarget::NativeReplacement {
            self.receipt.check_target_mode(self.mode)?;
            let path = &self.receipt.artifact;
            if path.file_name().is_some_and(is_companion_name) {
                check_native_companion(path)?;
            }
        } else {
            self.receipt.check_target()?;
        }
        self.receipt
            .invalidate(ExistingCompanions::PreserveForeign)?;
        self.held.check(&self.receipt.cancel)
    }

    pub(crate) fn invalidate_for_companion(&self) -> io::Result<()> {
        super::check(
            self.receipt.path(),
            self.receipt.receipt_protection,
            &self.receipt.cancel,
        )?;
        header::check_existing(
            self.receipt.path(),
            ReceiptOwner::File(&self.receipt.artifact_name),
        )?;
        self.invalidate()
    }

    pub(crate) fn finish_companion(
        self,
        out: &Path,
        owner: &OwnedFileToken,
        digest: OriginArtifactDigest,
        describe: impl FnOnce(&mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> Result<(), ConversionError> {
        let publication =
            self.receipt
                .publish_owned_companion(&self.held, digest, owner, || Ok(()), describe);
        let retirement = self.held.retire();
        match (publication, retirement) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(source), Ok(())) => Err(write_error(self.receipt.path(), source)),
            (Ok(()), Err(cleanup)) => Err(write_error(
                out,
                io::Error::other(LockFinishFailure::<ConversionError>::Committed { cleanup }),
            )),
            (Err(primary), Err(cleanup)) => Err(write_error(
                self.receipt.path(),
                io::Error::other(LockFinishFailure::<io::Error>::Receipt { primary, cleanup }),
            )),
        }
    }

    pub(crate) fn finish_conversion<T>(
        self,
        out: &Path,
        result: Result<T, ConversionError>,
    ) -> Result<T, ConversionError> {
        self.finish(result, Placement::Committed, |error| {
            write_error(out, io::Error::other(error))
        })
    }

    pub(crate) fn finish_extraction(
        self,
        out: &Path,
        result: Result<Outcome, ExtractionError>,
    ) -> Result<Outcome, ExtractionError> {
        let placement = if matches!(
            &result,
            Ok(Outcome::Written | Outcome::Converted(_) | Outcome::RawWritten(_))
        ) {
            Placement::Committed
        } else {
            Placement::Uncommitted
        };
        self.finish(result, placement, |error| {
            crate::write::write_error(out.to_path_buf(), io::Error::other(error))
        })
    }

    pub(crate) fn finish_io<T>(self, result: io::Result<T>) -> io::Result<T> {
        self.finish(result, Placement::Committed, io::Error::other)
    }

    fn finish<T, E: std::error::Error + 'static>(
        self,
        result: Result<T, E>,
        placement: Placement,
        map: impl FnOnce(LockFinishFailure<E>) -> E,
    ) -> Result<T, E> {
        let Err(cleanup) = self.held.retire() else {
            return result;
        };
        Err(map(match (result, placement) {
            (Ok(_), Placement::Committed) => LockFinishFailure::Committed { cleanup },
            (Ok(_), Placement::Uncommitted) => LockFinishFailure::Unplaced { cleanup },
            (Err(primary), _) => LockFinishFailure::Uncommitted { primary, cleanup },
        }))
    }
}

pub(super) fn check_native_companion(path: &Path) -> io::Result<()> {
    if is_owned_native_companion(path)? {
        return Err(super::invalid(
            "the native output path already holds an owned origin companion",
        ));
    }
    Ok(())
}

pub(crate) fn is_owned_native_companion(path: &Path) -> io::Result<bool> {
    if !path.file_name().is_some_and(is_companion_name) {
        return Ok(false);
    }
    let ownership = ExistingCompanions::PreserveForeign;
    if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(DIRECTORY_NAME))
    {
        ownership.owned(header::check_existing(path, ReceiptOwner::Directory))
    } else if let Some(stem) = path
        .file_stem()
        .and_then(|name| Path::new(name).file_stem())
    {
        let owner = crate::OriginArtifactName::from_basename(stem)?;
        ownership.owned(header::check_existing(path, ReceiptOwner::File(&owner)))
    } else {
        Ok(false)
    }
}

enum Placement {
    Committed,
    Uncommitted,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum LockFinishFailure<E> {
    #[error("output committed but its receipt and lock retirement failed: {cleanup}")]
    Receipt {
        #[source]
        primary: E,
        cleanup: io::Error,
    },
    #[error("output was not replaced and its temporary publication lock could not be retired")]
    Unplaced {
        #[source]
        cleanup: io::Error,
    },
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

    use super::{LockFinishFailure, OrdinaryReplacement, retained_lock_failure};
    use crate::file_receipt::lock::LEASE_NAME;
    use crate::write::Outcome;

    #[test]
    fn a_late_keep_reports_retirement_failure_without_claiming_replacement() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("output.bin");
        let lock = directory.path().join(LEASE_NAME);
        let replacement = OrdinaryReplacement::new(&output, None, None)?;
        fs::rename(&lock, directory.path().join("original-lock"))?;
        fs::write(&lock, b"foreign")?;

        let Err(error) = replacement.finish_extraction(&output, Ok(Outcome::Kept)) else {
            return Err(io::Error::other("the foreign lock cannot be retired"));
        };
        assert!(
            crate::guard_cancellation::retained_output(&error),
            "the cleanup issue remains observable"
        );
        let crate::ExtractionError::Write { source, .. } = error else {
            return Err(io::Error::other(error));
        };
        assert!(
            matches!(
                source.get_ref().and_then(|inner| inner.downcast_ref()),
                Some(LockFinishFailure::<crate::ExtractionError>::Unplaced { .. })
            ),
            "a kept outcome never reports successful replacement: {source:?}"
        );
        assert_eq!(fs::read(lock)?, b"foreign", "foreign lock untouched");
        Ok(())
    }

    #[test]
    fn retirement_distinguishes_commit_status_and_preserves_foreign_lock() -> io::Result<()> {
        for committed in [false, true] {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("output.bin");
            let lock = dir.path().join(LEASE_NAME);
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
            assert!(
                retained_lock_failure(&failed),
                "lock cleanup remains observable"
            );
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
