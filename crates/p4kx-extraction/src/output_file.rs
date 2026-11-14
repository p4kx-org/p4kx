// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;

use p4kx_texture::TextureError;

use crate::counting::{Counting, holds};
use crate::file_receipt::OrdinaryReplacement;
use crate::{
    ArchiveWriteProtection, CancelToken, ConversionError, OwnedFileToken, WriteProtectionError,
};

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
/// A committed replacement retires owned origin companions under the directory lock.
///
/// # Errors
/// Returns write failures or source/target protection refusals before creating or publishing.
pub fn write_replacing_protected<T>(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<T, ConversionError>,
) -> Result<T, ConversionError> {
    let mut held = None;
    let result = write_replacing_protected_before(out, protection, cancel, write, |staged| {
        let replacement = OrdinaryReplacement::new(out, protection, cancel)
            .map_err(|source| write_error(out, source))?;
        let replacement = held.insert(replacement);
        staged.check()?;
        replacement
            .invalidate()
            .map_err(|source| write_error(out, source))
    })
    .map(|(written, ())| written);
    if let Some(replacement) = held {
        replacement.finish_conversion(out, result)
    } else {
        result
    }
}

pub(crate) fn write_replacing_protected_before<T, Context>(
    out: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<T, ConversionError>,
    before_publish: impl FnOnce(&StagedOutput<'_>) -> Result<Context, ConversionError>,
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
                let staged = StagedOutput::new(&temporary, out, &ownership, protection, cancel);
                staged.check()?;
                let context = before_publish(&staged)?;
                staged.check()?;
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
                let staged = StagedOutput::new(&temporary, out, &ownership, protection, cancel);
                place(&staged, file, if_changed, len, crc, || {
                    OrdinaryReplacement::new(out, protection, cancel)
                        .map_err(|source| write_error(out, source))
                })
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

fn place<'source>(
    staged: &StagedOutput<'_>,
    file: File,
    if_changed: bool,
    len: u64,
    crc: u32,
    begin: impl FnOnce() -> Result<OrdinaryReplacement<'source>, ConversionError>,
) -> Result<Replaced, ConversionError> {
    staged.check()?;
    let out = staged.output;
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
    let replacement = begin()?;
    let result = (|| {
        staged.check()?;
        replacement
            .invalidate()
            .map_err(|source| write_error(out, source))?;
        staged.check()?;
        fs::rename(staged.temporary(), out).map_err(|source| write_error(out, source))?;
        Ok(Replaced { len, changed: true })
    })();
    replacement.finish_conversion(out, result)
}

pub(crate) struct StagedOutput<'a> {
    temporary: &'a Path,
    output: &'a Path,
    ownership: &'a OwnedFileToken,
    protection: Option<&'a ArchiveWriteProtection>,
    cancel: Option<&'a CancelToken>,
}

impl<'a> StagedOutput<'a> {
    pub(crate) const fn new(
        temporary: &'a Path,
        output: &'a Path,
        ownership: &'a OwnedFileToken,
        protection: Option<&'a ArchiveWriteProtection>,
        cancel: Option<&'a CancelToken>,
    ) -> Self {
        Self {
            temporary,
            output,
            ownership,
            protection,
            cancel,
        }
    }

    pub(crate) const fn temporary(&self) -> &Path {
        self.temporary
    }

    pub(crate) fn check(&self) -> Result<(), WriteProtectionError> {
        self.ownership
            .ensure_writable(self.temporary, cancel_flag(self.cancel))?;
        check_protected_output(self.output, self.protection, self.cancel)
    }
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
) -> Result<(), WriteProtectionError> {
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
#[path = "output_file_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "output_file_precommit_test.rs"]
mod precommit_test;
