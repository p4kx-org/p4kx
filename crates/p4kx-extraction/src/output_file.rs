// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;

use p4kx_texture::TextureError;

use crate::counting::{Counting, holds};
use crate::{ArchiveWriteProtection, CancelToken, ConversionError, OwnedFileToken};

const WRITE_BUFFER_LEN: usize = 256 * 1024;

/// Writes the file `out` through `write`, replacing it; the data goes to a temporary file next to
/// it first, so a failed write leaves nothing behind.
///
/// # Errors
///
/// Returns the error of `write`, and [`ConversionError::Write`] when the file cannot be created,
/// flushed or moved into place.
pub fn write_replacing<T>(
    out: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<T, ConversionError>,
) -> Result<T, ConversionError> {
    write_replacing_protected(out, None, None, write)
}

/// Atomically replaces `out` while retaining the Archive job's source protection.
///
/// # Errors
/// Returns write failures or source/target protection refusals before creating or publishing.
pub fn write_replacing_protected<T>(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<T, ConversionError>,
) -> Result<T, ConversionError> {
    write_replacing_protected_before(out, protection, cancel, write, |_| Ok(()))
        .map(|(written, ())| written)
}

pub(crate) fn write_replacing_protected_before<T, Context>(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<T, ConversionError>,
    before_publish: impl FnOnce(&Path) -> Result<Context, ConversionError>,
) -> Result<(T, Context), ConversionError> {
    check_protected_output(out, protection, cancel)?;
    let (temporary, file, ownership) = create_temporary(out, protection, cancel)?;
    let mut writer = BufWriter::with_capacity(WRITE_BUFFER_LEN, file);
    let written = write(&mut writer);
    let result = match written {
        Ok(value) => writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)
            .and_then(|file| file.sync_all())
            .map_err(|source| write_error(out, source))
            .and_then(|()| {
                ownership.ensure_writable(&temporary, cancel_flag(cancel))?;
                check_protected_output(out, protection, cancel)?;
                let context = before_publish(&temporary)?;
                ownership.ensure_writable(&temporary, cancel_flag(cancel))?;
                check_protected_output(out, protection, cancel)?;
                fs::rename(&temporary, out).map_err(|source| write_error(out, source))?;
                Ok((value, context))
            }),
        Err(error) => {
            drop(writer);
            Err(error)
        }
    };
    let result = result.map_err(|error| match error {
        ConversionError::Texture {
            source: TextureError::Write(source),
            ..
        } => write_error(out, source),
        other => other,
    });
    if result.is_err() {
        crate::output_cleanup::conversion(result, &ownership, &temporary, cancel)
    } else {
        result
    }
}

/// Writes `bytes` to the file `out` like [`write_replacing`].
///
/// # Errors
///
/// Returns the errors of [`write_replacing`].
pub fn write_bytes_replacing(out: &Path, bytes: &[u8]) -> Result<(), ConversionError> {
    write_replacing(out, |writer| {
        writer
            .write_all(bytes)
            .map_err(|source| write_error(out, source))
    })
}

/// How [`write_replacing_counted`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Replaced {
    /// The length of the data `write` produced.
    pub len: u64,
    /// Whether the file was put in place; `false` when it already held exactly that data.
    pub changed: bool,
}

/// Writes the file `out` through `write` like [`write_replacing`], streaming, and reports the
/// length written.
///
/// With `if_changed`, the data is compared by length and CRC-32C with an existing `out` once
/// written, and the temporary file is dropped instead of replacing a file that holds the same data.
///
/// # Errors
///
/// Returns the error of `write`, [`ConversionError::Write`] when the file cannot be created,
/// flushed or moved into place, and [`ConversionError::Read`] when an existing file cannot be read
/// for the comparison.
pub fn write_replacing_counted(
    out: &Path,
    if_changed: bool,
    write: impl FnOnce(&mut dyn Write) -> Result<(), ConversionError>,
) -> Result<Replaced, ConversionError> {
    write_replacing_counted_protected(out, if_changed, None, None, write)
}

/// Streams an atomic counted replacement under one Archive job's protection.
///
/// # Errors
/// Returns protection, write or comparison failures without replacing the prior output.
pub fn write_replacing_counted_protected(
    out: &Path,
    if_changed: bool,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut dyn Write) -> Result<(), ConversionError>,
) -> Result<Replaced, ConversionError> {
    check_protected_output(out, protection, cancel)?;
    let (temporary, file, ownership) = create_temporary(out, protection, cancel)?;
    let mut counting = Counting::new(BufWriter::with_capacity(WRITE_BUFFER_LEN, file));
    let written = write(&mut counting);
    let (len, crc) = (counting.len(), counting.crc());
    let result = match written {
        Ok(()) => counting
            .into_inner()
            .into_inner()
            .map_err(io::IntoInnerError::into_error)
            .map_err(|source| write_error(out, source))
            .and_then(|file| {
                ownership.ensure_writable(&temporary, cancel_flag(cancel))?;
                check_protected_output(out, protection, cancel)?;
                place(&temporary, file, out, if_changed, len, crc)
            }),
        Err(error) => {
            drop(counting);
            Err(error)
        }
    };
    if result.as_ref().map_or(true, |replaced| !replaced.changed) {
        return crate::output_cleanup::conversion(result, &ownership, &temporary, cancel);
    }
    result
}

fn place(
    temporary: &Path,
    file: File,
    out: &Path,
    if_changed: bool,
    len: u64,
    crc: u32,
) -> Result<Replaced, ConversionError> {
    if if_changed
        && holds(out, len, crc, &mut Vec::new()).map_err(|source| read_error(out, source))?
    {
        return Ok(Replaced {
            len,
            changed: false,
        });
    }
    file.sync_all().map_err(|source| write_error(out, source))?;
    drop(file);
    fs::rename(temporary, out).map_err(|source| write_error(out, source))?;
    Ok(Replaced { len, changed: true })
}

fn create_temporary(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
) -> Result<(PathBuf, File, OwnedFileToken), ConversionError> {
    let file_name = out
        .file_name()
        .ok_or_else(|| write_error(out, io::Error::from(io::ErrorKind::InvalidInput)))?;
    let mut name = file_name.to_os_string();
    name.push(format!(".p4kx-{}.part", process::id()));
    let temporary = out.with_file_name(name);
    check_protected_output(&temporary, protection, cancel)?;
    let (file, ownership) = crate::create_output_file(&temporary, protection, cancel_flag(cancel))
        .map_err(|error| match error {
            crate::OwnedOutputError::Io { source, .. } => write_error(out, source),
            other => other.into(),
        })?
        .into_parts();
    Ok((temporary, file, ownership))
}

/// Checks a leaf output using its job context, or target-only protection for independent data.
///
/// # Errors
/// Returns an established installation refusal or unavailable source/target identity.
pub fn check_protected_output(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
) -> Result<(), p4kx_guard::WriteProtectionError> {
    protection.map_or_else(
        || p4kx_guard::ensure_writable(path),
        |protection| protection.ensure_writable(path, cancel_flag(cancel)),
    )
}

pub(crate) fn cancel_flag(cancel: Option<&CancelToken>) -> &std::sync::atomic::AtomicBool {
    static NEVER_CANCELLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    cancel.map_or(&NEVER_CANCELLED, CancelToken::flag)
}

pub(crate) fn read_error(path: &Path, source: io::Error) -> ConversionError {
    ConversionError::Read {
        path: path.to_path_buf(),
        source,
    }
}

pub(crate) fn write_error(path: &Path, source: io::Error) -> ConversionError {
    ConversionError::Write {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::write_replacing_counted;

    fn names(folder: &std::path::Path) -> Vec<String> {
        fs::read_dir(folder)
            .expect("read folder")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn identical_data_is_dropped_under_if_changed_and_no_part_file_stays() {
        let dir = tempfile::tempdir().expect("folder");
        let out = dir.path().join("a.bin");
        let first = write_replacing_counted(&out, true, |writer| {
            writer
                .write_all(b"data")
                .map_err(|source| super::write_error(&out, source))
        })
        .expect("first");
        assert_eq!((first.len, first.changed), (4, true));
        let same = write_replacing_counted(&out, true, |writer| {
            writer
                .write_all(b"data")
                .map_err(|source| super::write_error(&out, source))
        })
        .expect("same");
        assert_eq!((same.len, same.changed), (4, false));
        let other = write_replacing_counted(&out, true, |writer| {
            writer
                .write_all(b"date")
                .map_err(|source| super::write_error(&out, source))
        })
        .expect("other");
        assert!(other.changed);
        assert_eq!(fs::read(&out).expect("read"), b"date");
        assert_eq!(names(dir.path()), ["a.bin"]);
    }

    #[test]
    fn overwrite_replaces_identical_data() {
        let dir = tempfile::tempdir().expect("folder");
        let out = dir.path().join("a.bin");
        for _ in 0..2 {
            let replaced = write_replacing_counted(&out, false, |writer| {
                writer
                    .write_all(b"data")
                    .map_err(|source| super::write_error(&out, source))
            })
            .expect("write");
            assert!(replaced.changed);
        }
    }

    #[test]
    fn a_failed_write_leaves_nothing_behind() {
        let dir = tempfile::tempdir().expect("folder");
        let out = dir.path().join("a.bin");
        let failed = write_replacing_counted(&out, true, |writer| {
            writer
                .write_all(b"half")
                .map_err(|source| super::write_error(&out, source))?;
            Err(super::write_error(&out, std::io::Error::other("stop")))
        });
        assert!(failed.is_err());
        assert_eq!(fs::read_dir(dir.path()).expect("read").count(), 0);
    }

    #[test]
    fn cancellation_removes_a_fresh_ordinary_temporary_and_preserves_the_target() {
        use std::io::Write;

        let dir = tempfile::tempdir().expect("folder");
        let source = dir.path().join("source.p4k");
        let out = dir.path().join("result.bin");
        fs::write(&source, b"source").expect("source");
        fs::write(&out, b"prior").expect("prior output");
        let cancel = crate::CancelToken::default();
        let protection =
            crate::ArchiveWriteProtection::new(&source, cancel.flag()).expect("context");
        let result =
            super::write_replacing_protected(&out, Some(&protection), Some(&cancel), |writer| {
                writer
                    .write_all(b"unpublished")
                    .map_err(|error| super::write_error(&out, error))?;
                cancel.cancel();
                Ok(())
            });
        assert!(matches!(result, Err(crate::ConversionError::Cancelled)));
        assert_eq!(fs::read(&out).expect("output"), b"prior");
        assert_eq!(fs::read(&source).expect("source"), b"source");
        assert_eq!(names(dir.path()).len(), 2);
    }

    #[test]
    fn a_retargeted_temporary_is_retained_as_a_failure_during_cancellation() {
        let dir = tempfile::tempdir().expect("folder");
        let source = dir.path().join("source.p4k");
        let out = dir.path().join("result.bin");
        let temporary = out.with_file_name(format!("result.bin.p4kx-{}.part", std::process::id()));
        let retained = dir.path().join("original.part");
        fs::write(&source, b"source").expect("source");
        let cancel = crate::CancelToken::default();
        let protection =
            crate::ArchiveWriteProtection::new(&source, cancel.flag()).expect("context");
        let result =
            super::write_replacing_protected(&out, Some(&protection), Some(&cancel), |_| {
                fs::rename(&temporary, &retained).expect("retarget owned namespace");
                fs::hard_link(&source, &temporary).expect("source alias");
                cancel.cancel();
                Ok(())
            });
        assert!(matches!(
            result,
            Err(crate::ConversionError::CleanupFailed { .. })
        ));
        assert_eq!(fs::read(&source).expect("source"), b"source");
        assert_eq!(
            fs::read(&temporary).expect("unowned alias remains"),
            b"source"
        );
        assert!(retained.is_file());
        assert!(!out.exists());
    }
}

#[cfg(test)]
#[path = "output_file_precommit_test.rs"]
mod precommit_test;
