// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};

use p4kx_cryxml::{Document, Format, Signature, detect, parse};

use crate::{Archive, CancelToken, Entry, LineDiffError};

/// The largest Entry, and the largest XML converted from `CryXml`, a line diff reads.
pub const MAX_DIFF_LEN: u64 = 128 * 1024 * 1024;

/// The most lines a side of a line diff may have; with about 9 bytes of bookkeeping per line and
/// side this bounds the memory of a diff at well under 100 MiB beyond the texts.
pub const MAX_DIFF_LINES: usize = 4 * 1024 * 1024;

/// The most a buffer reserves before its data has arrived; it grows from there.
const MAX_INITIAL_CAPACITY: u64 = 16 * 1024 * 1024;

const UTF16_LE_BOM: &[u8] = b"\xFF\xFE";
const UTF16_BE_BOM: &[u8] = b"\xFE\xFF";

/// How one side of a line diff was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// Bytes without a NUL, diffed as stored.
    Text,
    /// UTF-16 little-endian after its byte order mark, decoded to UTF-8.
    Utf16Le,
    /// UTF-16 big-endian after its byte order mark, decoded to UTF-8.
    Utf16Be,
    /// `CryXml` with this signature, converted to XML before diffing.
    CryXml(Signature),
}

impl Encoding {
    /// Returns the name the JSON output uses: `text`, `utf-16le`, `utf-16be` or the `CryXml`
    /// signature.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::CryXml(signature) => signature.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeOutcome {
    Text(Encoding),
    Binary,
    TooLarge(LimitSource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LimitSource {
    DeclaredSize,
    DecodedPayload,
}

/// A side's text together with the parsed source needed for selected CryXml hunk evidence.
#[derive(Debug)]
pub(crate) struct DecodedText<'raw> {
    pub(crate) bytes: Vec<u8>,
    encoding: Encoding,
    pub(crate) cryxml: Option<Box<Document<'raw>>>,
}

/// One side of a line diff after decoding it.
#[derive(Debug)]
pub(crate) enum Decoded<'raw> {
    Text(DecodedText<'raw>),
    Binary,
    TooLarge(LimitSource),
}

impl Decoded<'_> {
    pub(crate) const fn outcome(&self) -> DecodeOutcome {
        match self {
            Self::Text(text) => DecodeOutcome::Text(text.encoding),
            Self::Binary => DecodeOutcome::Binary,
            Self::TooLarge(source) => DecodeOutcome::TooLarge(*source),
        }
    }
}

/// A physical Entry read whose bytes remain available to the CryXml document.
pub(crate) struct LoadedEntry<'archive> {
    entry: &'archive Entry,
    bytes: Option<Vec<u8>>,
}

impl LoadedEntry<'_> {
    pub(crate) fn decode(
        &mut self,
        cancel: Option<&CancelToken>,
    ) -> Result<Decoded<'_>, LineDiffError> {
        let Some(bytes) = self.bytes.as_mut() else {
            return Ok(Decoded::TooLarge(LimitSource::DeclaredSize));
        };
        let decoded = match detect(bytes) {
            Some(Format::CryXml(signature)) => convert(bytes, signature, self.entry, cancel)?,
            _ => text(std::mem::take(bytes)),
        };
        Ok(match decoded {
            Decoded::Text(text) if line_count(&text.bytes) > MAX_DIFF_LINES => {
                Decoded::TooLarge(LimitSource::DecodedPayload)
            }
            decoded => decoded,
        })
    }
}

/// Reads the verified original payload unless its declared size exceeds the line diff limit.
pub(crate) fn load<'archive>(
    archive: &Archive,
    entry: &'archive Entry,
    cancel: Option<&CancelToken>,
) -> Result<LoadedEntry<'archive>, LineDiffError> {
    let declared = entry.uncompressed_size();
    if declared > MAX_DIFF_LEN {
        return Ok(LoadedEntry { entry, bytes: None });
    }
    let path = || entry.path().to_owned();
    let mut bytes = Vec::with_capacity(initial_capacity(declared));
    let cancelled = || cancel.is_some_and(CancelToken::is_cancelled);
    Watched {
        inner: archive.open_entry(entry)?,
        cancel,
    }
    .take(declared.saturating_add(1))
    .read_to_end(&mut bytes)
    .map_err(|source| {
        if cancelled() {
            LineDiffError::Cancelled
        } else {
            LineDiffError::Read {
                path: path(),
                source,
            }
        }
    })?;
    if bytes.len() as u64 != declared {
        return Err(LineDiffError::SizeMismatch {
            path: path(),
            declared,
        });
    }
    Ok(LoadedEntry {
        entry,
        bytes: Some(bytes),
    })
}

fn convert<'raw>(
    bytes: &'raw [u8],
    signature: Signature,
    entry: &Entry,
    cancel: Option<&CancelToken>,
) -> Result<Decoded<'raw>, LineDiffError> {
    let document = parse(bytes).map_err(|source| LineDiffError::CryXml {
        path: entry.path().to_owned(),
        source,
    })?;
    let mut out = Capped {
        bytes: Vec::with_capacity(initial_capacity((bytes.len() as u64).saturating_mul(2))),
        cancel,
    };
    match document.write_xml(&mut out) {
        Ok(()) => Ok(Decoded::Text(DecodedText {
            bytes: out.bytes,
            encoding: Encoding::CryXml(signature),
            cryxml: Some(Box::new(document)),
        })),
        Err(_) if cancel.is_some_and(CancelToken::is_cancelled) => Err(LineDiffError::Cancelled),
        Err(error) if error.kind() == io::ErrorKind::FileTooLarge => {
            Ok(Decoded::TooLarge(LimitSource::DecodedPayload))
        }
        Err(source) => Err(LineDiffError::Read {
            path: entry.path().to_owned(),
            source,
        }),
    }
}

/// Returns `bytes` as text: strict UTF-16 after its byte order mark, otherwise the bytes as they
/// are; binary when they hold a NUL, or when UTF-16 holds a NUL, an unpaired surrogate or an odd
/// trailing byte.
fn text(bytes: Vec<u8>) -> Decoded<'static> {
    let utf16 = bytes
        .strip_prefix(UTF16_LE_BOM)
        .map(|units| (utf16(units, u16::from_le_bytes), Encoding::Utf16Le))
        .or_else(|| {
            bytes
                .strip_prefix(UTF16_BE_BOM)
                .map(|units| (utf16(units, u16::from_be_bytes), Encoding::Utf16Be))
        });
    match utf16 {
        Some((Some(text), encoding)) => Decoded::Text(DecodedText {
            bytes: text.into_bytes(),
            encoding,
            cryxml: None,
        }),
        Some((None, _)) => Decoded::Binary,
        None if bytes.contains(&0) => Decoded::Binary,
        None => Decoded::Text(DecodedText {
            bytes,
            encoding: Encoding::Text,
            cryxml: None,
        }),
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> Option<String> {
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let text: String = char::decode_utf16(pairs.iter().map(|&pair| unit(pair)))
        .collect::<Result<_, _>>()
        .ok()?;
    (!text.contains('\0')).then_some(text)
}

fn line_count(text: &[u8]) -> usize {
    text.split_inclusive(|&byte| byte == b'\n').count()
}

fn initial_capacity(expected: u64) -> usize {
    usize::try_from(expected.min(MAX_INITIAL_CAPACITY)).unwrap_or(0)
}

/// Fails a read once the token is set.
struct Watched<'c, R> {
    inner: R,
    cancel: Option<&'c CancelToken>,
}

impl<R: Read> Read for Watched<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(io::Error::other(LineDiffError::Cancelled));
        }
        self.inner.read(buf)
    }
}

/// Collects written bytes up to [`MAX_DIFF_LEN`] and fails with
/// [`io::ErrorKind::FileTooLarge`] beyond it, or once the token is set.
struct Capped<'c> {
    bytes: Vec<u8>,
    cancel: Option<&'c CancelToken>,
}

impl Write for Capped<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(io::Error::other(LineDiffError::Cancelled));
        }
        let fits = u64::try_from(self.bytes.len().saturating_add(buf.len()))
            .is_ok_and(|len| len <= MAX_DIFF_LEN);
        if !fits {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
