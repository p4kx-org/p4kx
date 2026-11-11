// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;

use p4kx_extraction::{ArchiveWriteProtection, CancelToken, check_protected_output};

/// Calls `write` with a new temporary file next to `out` and renames it to `out` once `write`
/// succeeded, replacing an existing file; on any error the temporary file is removed, so `out`
/// is never half written.
///
/// # Errors
///
/// Returns the [`io::Error`] of `write`, or of creating, flushing or renaming the file.
pub(crate) fn write_replacing<T>(
    out: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> io::Result<T>,
) -> io::Result<T> {
    write_replacing_protected(out, None, None, write)
}

/// Atomically writes under the source protection retained by one Archive job.
pub(crate) fn write_replacing_protected<T>(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut BufWriter<File>) -> io::Result<T>,
) -> io::Result<T> {
    write_replacing_before(out, protection, cancel, write, || Ok(()))
}

/// Stages a replacement and runs one guarded action just before the final rename.
pub(crate) fn write_replacing_before<T>(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut BufWriter<File>) -> io::Result<T>,
    before_publish: impl FnOnce() -> io::Result<()>,
) -> io::Result<T> {
    let uncancelled = CancelToken::new();
    let actual_cancel = cancel.unwrap_or(&uncancelled);
    let check = |path: &Path| {
        check_protected_output(path, protection, cancel)
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))
    };
    check(out)?;
    let temporary = temporary_path(out)?;
    check(&temporary)?;
    let (file, ownership) =
        p4kx_extraction::create_output_file(&temporary, protection, actual_cancel.flag())
            .map_err(p4kx_extraction::OwnedOutputError::into_io_error)?
            .into_parts();
    let mut writer = BufWriter::new(file);
    let result = write(&mut writer).and_then(|value| {
        writer.flush()?;
        writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
        ownership
            .ensure_writable(&temporary, actual_cancel.flag())
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
        check(out)?;
        before_publish()?;
        ownership
            .ensure_writable(&temporary, actual_cancel.flag())
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
        check(out)?;
        fs::rename(&temporary, out)?;
        Ok(value)
    });
    if let Err(write) = result {
        let cleanup = ownership.remove(&temporary, actual_cancel.flag());
        return Err(match cleanup {
            Ok(()) => write,
            Err(cleanup) => p4kx_extraction::OwnedOutputError::Cleanup {
                primary: Box::new(p4kx_extraction::OwnedOutputError::Io {
                    path: out.to_path_buf(),
                    source: write,
                }),
                cleanup,
            }
            .into_io_error(),
        });
    }
    result
}

/// Returns `<name>.p4kx-<process id>.part` next to `out`.
fn temporary_path(out: &Path) -> io::Result<PathBuf> {
    let name = out
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the output names no file"))?;
    let mut name = name.to_os_string();
    name.push(format!(".p4kx-{}.part", process::id()));
    Ok(out.with_file_name(name))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write as _;

    use p4kx_extraction::{ArchiveWriteProtection, CancelToken, WriteProtectionError};

    use super::write_replacing_protected;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn an_archive_hard_link_created_before_commit_is_not_overwritten() -> TestResult {
        let root = tempfile::tempdir()?;
        let source = root.path().join("Data.p4k");
        let out = root.path().join("report.txt");
        fs::write(&source, b"synthetic source bytes")?;
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
        let failure = write_replacing_protected(&out, Some(&protection), Some(&cancel), |writer| {
            writer.write_all(b"output")?;
            fs::hard_link(&source, &out)
        })
        .expect_err("final target is the source inode");
        assert!(matches!(
            failure
                .get_ref()
                .and_then(|cause| cause.downcast_ref::<WriteProtectionError>()),
            Some(WriteProtectionError::SourceOverwrite { .. })
        ));
        assert_eq!(fs::read(&source)?, b"synthetic source bytes");
        assert_eq!(fs::read(&out)?, b"synthetic source bytes");
        assert_eq!(
            fs::read_dir(root.path())?.count(),
            2,
            "owned temporary file is removed"
        );
        Ok(())
    }

    #[test]
    fn the_original_source_name_is_refused_before_the_write_callback() -> TestResult {
        let root = tempfile::tempdir()?;
        let source = root.path().join("Data.p4k");
        fs::write(&source, b"synthetic source bytes")?;
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
        let mut called = false;
        let result = write_replacing_protected(&source, Some(&protection), Some(&cancel), |_| {
            called = true;
            Ok(())
        });
        assert!(result.is_err());
        assert!(!called);
        assert_eq!(fs::read(&source)?, b"synthetic source bytes");
        Ok(())
    }

    #[test]
    fn an_ordinary_cancelled_write_cleans_its_owned_file_and_keeps_prior_output() -> TestResult {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source.p4k");
        let out = root.path().join("report.txt");
        fs::write(&source, b"source")?;
        fs::write(&out, b"prior")?;
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
        let result = write_replacing_protected(&out, Some(&protection), Some(&cancel), |writer| {
            writer.write_all(b"unpublished")?;
            cancel.cancel();
            Ok(())
        });
        let failure = result.expect_err("cancel before publication");
        assert!(
            failure
                .get_ref()
                .and_then(|error| error.downcast_ref::<WriteProtectionError>())
                .is_some_and(WriteProtectionError::is_cancelled)
        );
        assert_eq!(fs::read(&out)?, b"prior");
        assert_eq!(fs::read_dir(root.path())?.count(), 2);
        Ok(())
    }

    #[test]
    fn a_cancelled_retarget_keeps_the_typed_cleanup_failure() -> TestResult {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source.p4k");
        let out = root.path().join("report.txt");
        let temporary = super::temporary_path(&out)?;
        fs::write(&source, b"source")?;
        let cancel = CancelToken::new();
        let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
        let result = write_replacing_protected(&out, Some(&protection), Some(&cancel), |_| {
            fs::rename(&temporary, root.path().join("retained.bin"))?;
            fs::hard_link(&source, &temporary)?;
            cancel.cancel();
            Ok(())
        });
        let failure = result.expect_err("retained temporary");
        assert!(matches!(
            p4kx_extraction::owned_output_failure(&failure),
            Some(p4kx_extraction::OwnedOutputError::Cleanup { .. })
        ));
        assert_eq!(fs::read(&source)?, b"source");
        assert_eq!(fs::read(&temporary)?, b"source");
        assert!(!out.exists());
        Ok(())
    }
}
