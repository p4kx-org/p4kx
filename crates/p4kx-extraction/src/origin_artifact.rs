// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Component, Path};

use p4kx_archive::Checksum;
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::DecimalU64;

/// One relative artifact basename in its exact native spelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct OriginArtifactName(ArtifactName);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(
    tag = "encoding",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
enum ArtifactName {
    Utf8(#[schemars(regex(pattern = r"^(?!\.{1,2}$)[^/\\:\u0000]+$"))] String),
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "Portable readers accept native Unix names")
    )]
    UnixBytes(Vec<u8>),
    #[cfg_attr(
        not(windows),
        expect(dead_code, reason = "Portable readers accept native Windows names")
    )]
    WindowsUtf16(Vec<u16>),
}

impl OriginArtifactName {
    /// Records the exact portable or native representation of a relative basename.
    ///
    /// # Errors
    /// Returns invalid input when the name contains a path or a Windows stream suffix.
    pub fn from_basename(name: &OsStr) -> io::Result<Self> {
        let mut parts = Path::new(name).components();
        if !matches!(parts.next(), Some(Component::Normal(part)) if part == name)
            || parts.next().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "origin name is not a basename",
            ));
        }
        let name = match name.to_str() {
            Some(text) if !text.contains(['\\', ':', '\0']) => ArtifactName::Utf8(text.to_owned()),
            _ => native_name(name)?,
        };
        Ok(Self(name))
    }
}

#[cfg(unix)]
fn native_name(name: &OsStr) -> io::Result<ArtifactName> {
    use std::os::unix::ffi::OsStrExt as _;

    if name.as_bytes().contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "origin name contains a null byte",
        ));
    }
    Ok(ArtifactName::UnixBytes(name.as_bytes().to_vec()))
}

#[cfg(windows)]
fn native_name(name: &OsStr) -> io::Result<ArtifactName> {
    use std::os::windows::ffi::OsStrExt as _;

    let name: Vec<_> = name.encode_wide().collect();
    if name
        .iter()
        .any(|&unit| unit == 0 || unit == u16::from(b':'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "origin name is not a Windows filename",
        ));
    }
    Ok(ArtifactName::WindowsUtf16(name))
}

/// Actual written byte length and SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginArtifactDigest {
    byte_length: DecimalU64,
    #[schemars(regex(pattern = "^[0-9a-f]{64}$"))]
    sha256: String,
}

/// A stream that measures only bytes accepted by its actual sink.
pub struct OriginDigestWriter<W> {
    writer: W,
    digest: Sha256,
    bytes: u64,
}

impl<W: Write> OriginDigestWriter<W> {
    /// Wraps the selected artifact sink.
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            digest: Sha256::new(),
            bytes: 0,
        }
    }

    /// Finishes the byte binding after the sink accepts its buffered bytes.
    ///
    /// # Errors
    /// Returns an error when the sink cannot flush.
    pub fn finish(mut self) -> io::Result<OriginArtifactDigest> {
        self.flush()?;
        Ok(OriginArtifactDigest {
            byte_length: DecimalU64::new(self.bytes),
            sha256: Checksum::Sha256(self.digest.finalize().into()).hex(),
        })
    }
}

impl<W: Write> Write for OriginDigestWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.writer.write(bytes)?;
        let actual = bytes
            .get(..written)
            .ok_or_else(|| io::Error::other("artifact sink returned an invalid byte count"))?;
        self.bytes = self
            .bytes
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("artifact byte count exceeds u64"))?;
        self.digest.update(actual);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
