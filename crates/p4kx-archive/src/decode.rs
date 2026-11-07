// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Chain, Cursor, Read};
use std::mem;

use ruzstd::decoding::{FrameDecoder, StreamingDecoder};

use crate::decrypt::AES_BLOCK_LEN;
use crate::{Compression, DataError, EffectiveEncoding};

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
const MIN_ZSTD_WINDOW: u64 = 128 * 1024 * 1024;

type Prefixed<R> = Chain<Cursor<Vec<u8>>, R>;

enum ZstdStage<R: Read> {
    Start(R),
    Frame(Box<StreamingDecoder<Prefixed<R>, FrameDecoder>>),
    Plain(Prefixed<R>),
    Failed,
}

/// Decodes one zstd frame, or passes data without the zstd magic through unchanged.
pub(crate) struct ZstdOrPlain<R: Read> {
    stage: ZstdStage<R>,
    max_window: u64,
}

impl<R: Read> ZstdOrPlain<R> {
    /// Wraps `inner`, allowing a window up to the Entry's uncompressed size.
    pub(crate) fn new(inner: R, uncompressed_size: u64) -> Self {
        Self {
            stage: ZstdStage::Start(inner),
            max_window: uncompressed_size.max(MIN_ZSTD_WINDOW),
        }
    }

    pub(crate) const fn effective_encoding(&self) -> EffectiveEncoding {
        match self.stage {
            ZstdStage::Start(_) | ZstdStage::Failed => EffectiveEncoding::Undetermined,
            ZstdStage::Frame(_) => EffectiveEncoding::Zstandard,
            ZstdStage::Plain(_) => EffectiveEncoding::PassThrough,
        }
    }

    fn begin(&mut self) -> io::Result<()> {
        let ZstdStage::Start(mut inner) = mem::replace(&mut self.stage, ZstdStage::Failed) else {
            return Ok(());
        };
        let prefix = read_prefix(&mut inner)?;
        let is_frame = prefix == ZSTD_MAGIC;
        let prefixed = Cursor::new(prefix).chain(inner);
        self.stage = if is_frame {
            let decoder = StreamingDecoder::new_with_max_window_size(prefixed, self.max_window)
                .map_err(io::Error::other)?;
            ZstdStage::Frame(Box::new(decoder))
        } else {
            ZstdStage::Plain(prefixed)
        };
        Ok(())
    }
}

impl<R: Read> Read for ZstdOrPlain<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if matches!(self.stage, ZstdStage::Start(_)) {
            self.begin()?;
        }
        match &mut self.stage {
            ZstdStage::Frame(decoder) => decoder.read(buffer),
            ZstdStage::Plain(plain) => plain.read(buffer),
            ZstdStage::Start(_) | ZstdStage::Failed => Err(io::Error::other(
                "zstd data cannot be read after an earlier error",
            )),
        }
    }
}

fn read_prefix(inner: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut prefix = Vec::with_capacity(ZSTD_MAGIC.len());
    inner
        .take(ZSTD_MAGIC.len() as u64)
        .read_to_end(&mut prefix)?;
    Ok(prefix)
}

/// Reports the decoding errors of a decompressor as [`DataError::Corrupt`] for its compression.
pub(crate) struct CorruptAs<R> {
    inner: R,
    compression: Compression,
}

impl<R> CorruptAs<R> {
    pub(crate) const fn inner(&self) -> &R {
        &self.inner
    }

    pub(crate) const fn new(inner: R, compression: Compression) -> Self {
        Self { inner, compression }
    }
}

impl<R: Read> Read for CorruptAs<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer).map_err(|source| {
            if matches!(
                source.kind(),
                io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput | io::ErrorKind::Other
            ) {
                DataError::Corrupt {
                    compression: self.compression,
                    source,
                }
                .into()
            } else {
                source
            }
        })
    }
}

/// What [`ExactSize`] does with data beyond the uncompressed size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Overlong {
    /// Cut it.
    Cut,
    /// Report it as [`DataError::TooLong`]; with `padding`, fewer than one AES block of zero bytes
    /// is the padding of an encrypted Entry and allowed.
    Reject {
        /// Whether zero padding below one AES block is allowed.
        padding: bool,
    },
}

/// Yields exactly the uncompressed size: shorter data is an error, longer data is cut or an
/// error by its [`Overlong`] mode.
pub(crate) struct ExactSize<R> {
    inner: R,
    produced: u64,
    expected: u64,
    overlong: Overlong,
}

impl<R> ExactSize<R> {
    pub(crate) const fn inner(&self) -> &R {
        &self.inner
    }

    pub(crate) const fn new(inner: R, expected: u64) -> Self {
        Self {
            inner,
            produced: 0,
            expected,
            overlong: Overlong::Cut,
        }
    }

    pub(crate) const fn set_overlong(&mut self, overlong: Overlong) {
        self.overlong = overlong;
    }
}

impl<R: Read> ExactSize<R> {
    /// Fails when the data goes on after the expected size, once.
    fn check_end(&mut self) -> io::Result<()> {
        let Overlong::Reject { padding } = self.overlong else {
            return Ok(());
        };
        self.overlong = Overlong::Cut;
        let mut probe = [0_u8; AES_BLOCK_LEN];
        let mut extra = 0;
        while let Some(rest) = probe.get_mut(extra..).filter(|rest| !rest.is_empty()) {
            let read = self.inner.read(rest)?;
            if read == 0 {
                break;
            }
            extra += read;
        }
        let tail = probe.get(..extra).unwrap_or_default();
        let padded = padding && extra < AES_BLOCK_LEN && tail.iter().all(|&byte| byte == 0);
        if extra == 0 || padded {
            Ok(())
        } else {
            Err(DataError::TooLong {
                expected: self.expected,
            }
            .into())
        }
    }
}

impl<R: Read> Read for ExactSize<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.expected - self.produced;
        let count = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let Some(window) = buffer.get_mut(..count).filter(|window| !window.is_empty()) else {
            if remaining == 0 && !buffer.is_empty() {
                self.check_end()?;
            }
            return Ok(0);
        };
        let read = self.inner.read(window)?;
        if read == 0 {
            return Err(DataError::TooShort {
                actual: self.produced,
                expected: self.expected,
            }
            .into());
        }
        self.produced += read as u64;
        Ok(read)
    }
}
